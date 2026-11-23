use std::any::{Any, TypeId};
use std::collections::BTreeMap;

use super::World;
use crate::component_storage::ComponentStorage;
use crate::{AccessSet, Entity, Res, ResMut, ScopeError, SparseSet, StorageKind, SystemAccess};

/// A system's borrowed storage for one batch.
pub struct SystemScope<'world> {
    /// The component types declared by the system.
    pub components: ScopedComponents<'world>,
    /// The resource types declared by the system.
    pub resources: ScopedResources<'world>,
}

/// Component access restricted to one system's declarations.
pub struct ScopedComponents<'world> {
    access: &'world AccessSet,
    stores: Stores<'world, dyn ComponentStorage>,
    tick: u64,
}

impl ScopedComponents<'_> {
    /// Reads an entity's component if it is present.
    ///
    /// # Errors
    /// Returns an error if the system has no access to the type.
    pub fn get<T: Send + Sync + 'static>(&self, entity: Entity) -> Result<Option<&T>, ScopeError> {
        let id = TypeId::of::<T>();
        ensure(self.access, StorageKind::Component, id, false)?;
        Ok(self.stores.get(&id).and_then(|store| {
            let erased: &dyn Any = store.get();
            erased.downcast_ref::<SparseSet<T>>()?.get(entity)
        }))
    }

    /// Mutably borrows an entity's component and marks it changed.
    ///
    /// # Errors
    /// Returns an error if the system has no write access to the type.
    pub fn get_mut<T: Send + Sync + 'static>(
        &mut self,
        entity: Entity,
    ) -> Result<Option<&mut T>, ScopeError> {
        let id = TypeId::of::<T>();
        ensure(self.access, StorageKind::Component, id, true)?;
        Ok(self.stores.get_mut(&id).and_then(|store| {
            let erased: &mut dyn Any = store.get_mut()?;
            erased
                .downcast_mut::<SparseSet<T>>()?
                .get_mut_at_tick(entity, self.tick)
        }))
    }
}

/// Resource access restricted to one system's declarations.
pub struct ScopedResources<'world> {
    access: &'world AccessSet,
    stores: Stores<'world, dyn Any + Send + Sync>,
}

impl ScopedResources<'_> {
    /// Borrows the resource if present.
    ///
    /// # Errors
    /// Returns an error if the system has no access to the type.
    pub fn get<T: Send + Sync + 'static>(&self) -> Result<Option<Res<'_, T>>, ScopeError> {
        let id = TypeId::of::<T>();
        ensure(self.access, StorageKind::Resource, id, false)?;
        Ok(self
            .stores
            .get(&id)
            .and_then(|store| store.get().downcast_ref())
            .map(Res))
    }

    /// Mutably borrows the resource if present.
    ///
    /// # Errors
    /// Returns an error if the system has no write access to the type.
    pub fn get_mut<T: Send + Sync + 'static>(
        &mut self,
    ) -> Result<Option<ResMut<'_, T>>, ScopeError> {
        let id = TypeId::of::<T>();
        ensure(self.access, StorageKind::Resource, id, true)?;
        Ok(self
            .stores
            .get_mut(&id)
            .and_then(Borrowed::get_mut)
            .and_then(|value| value.downcast_mut())
            .map(ResMut))
    }
}

impl World {
    /// Splits storage into system scopes in input order.
    ///
    /// Scopes keep structural changes outside the batch by borrowing the world.
    ///
    /// # Errors
    /// Returns an error if the declarations contain conflicting access.
    pub fn scope_batch<'world>(
        &'world mut self,
        systems: impl IntoIterator<Item = &'world SystemAccess>,
    ) -> Result<Vec<SystemScope<'world>>, ScopeError> {
        let tick = self.change_tick();
        let systems: Vec<_> = systems.into_iter().collect();
        let components = split(
            StorageKind::Component,
            systems.iter().map(|system| system.components()),
            self.components
                .iter_mut()
                .map(|(&id, storage)| (id, storage.as_mut())),
        )?;
        let resources = split(
            StorageKind::Resource,
            systems.iter().map(|system| system.resources()),
            self.resources.entries_mut(),
        )?;
        Ok(systems
            .into_iter()
            .zip(components)
            .zip(resources)
            .map(|((access, components), resources)| {
                for &id in access.components().writes() {
                    self.transforms.invalidate(id);
                }
                SystemScope {
                    components: ScopedComponents {
                        access: access.components(),
                        stores: components,
                        tick,
                    },
                    resources: ScopedResources {
                        access: access.resources(),
                        stores: resources,
                    },
                }
            })
            .collect())
    }
}

fn ensure(
    access: &AccessSet,
    storage: StorageKind,
    type_id: TypeId,
    write: bool,
) -> Result<(), ScopeError> {
    if access.writes().contains(&type_id) || (!write && access.reads().contains(&type_id)) {
        Ok(())
    } else {
        Err(ScopeError::UndeclaredAccess {
            storage,
            type_id,
            write,
        })
    }
}

enum Borrowed<'world, T: ?Sized> {
    Shared(&'world T),
    Exclusive(&'world mut T),
}

impl<T: ?Sized> Borrowed<'_, T> {
    fn get(&self) -> &T {
        match self {
            Self::Shared(value) => value,
            Self::Exclusive(value) => value,
        }
    }

    fn get_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Shared(_) => None,
            Self::Exclusive(value) => Some(value),
        }
    }
}

type Stores<'world, T> = BTreeMap<TypeId, Borrowed<'world, T>>;

enum Borrowers {
    Shared(Vec<usize>),
    Exclusive(usize),
}

fn split<'world, T: ?Sized>(
    storage: StorageKind,
    access: impl IntoIterator<Item = &'world AccessSet>,
    entries: impl IntoIterator<Item = (TypeId, &'world mut T)>,
) -> Result<Vec<Stores<'world, T>>, ScopeError> {
    let mut requests = BTreeMap::new();
    let mut scopes = Vec::new();
    for (system, access) in access.into_iter().enumerate() {
        scopes.push(BTreeMap::new());
        for &type_id in access.reads() {
            let entry = requests
                .entry(type_id)
                .or_insert_with(|| Borrowers::Shared(Vec::new()));
            let Borrowers::Shared(readers) = entry else {
                return Err(ScopeError::BatchConflict { storage, type_id });
            };
            readers.push(system);
        }
        for &type_id in access.writes() {
            if requests
                .insert(type_id, Borrowers::Exclusive(system))
                .is_some()
            {
                return Err(ScopeError::BatchConflict { storage, type_id });
            }
        }
    }

    for (id, value) in entries {
        match requests.remove(&id) {
            Some(Borrowers::Shared(readers)) => {
                for reader in readers {
                    if let Some(scope) = scopes.get_mut(reader) {
                        scope.insert(id, Borrowed::Shared(&*value));
                    }
                }
            }
            Some(Borrowers::Exclusive(writer)) => {
                if let Some(scope) = scopes.get_mut(writer) {
                    scope.insert(id, Borrowed::Exclusive(value));
                }
            }
            None => {}
        }
    }
    Ok(scopes)
}
