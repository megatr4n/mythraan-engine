use std::any::{Any, TypeId, type_name};
use std::collections::{BTreeMap, btree_map::Entry};

use crate::component_storage::ComponentStorage;
use crate::{Entity, EntityAllocError, EntityAllocator, SparseSet, WorldInsertError};

/// Owns the entities in a simulation.
///
/// Use entity identifiers with the world that issued them.
#[derive(Default)]
pub struct World {
    entities: EntityAllocator,
    components: BTreeMap<TypeId, Box<dyn ComponentStorage>>,
}

impl World {
    /// Creates an empty world.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an entity with no components.
    ///
    /// # Errors
    ///
    /// Returns an error if the allocator cannot issue another identifier.
    pub fn spawn(&mut self) -> Result<Entity, EntityAllocError> {
        self.entities.spawn()
    }

    /// Returns whether this entity identifier is live in the world.
    #[must_use]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    /// Inserts a component and returns the previous value of the same type, if any.
    ///
    /// # Errors
    ///
    /// Returns an error if the entity is not live or the component cannot be stored.
    pub fn insert<T: Send + Sync + 'static>(
        &mut self,
        entity: Entity,
        component: T,
    ) -> Result<Option<T>, WorldInsertError> {
        if !self.is_alive(entity) {
            return Err(WorldInsertError::EntityNotAlive { entity });
        }

        match self.components.entry(TypeId::of::<T>()) {
            Entry::Occupied(mut entry) => {
                let erased: &mut dyn Any = entry.get_mut().as_mut();
                let storage = erased.downcast_mut::<SparseSet<T>>().ok_or(
                    WorldInsertError::StorageTypeMismatch {
                        component: type_name::<T>(),
                    },
                )?;
                Ok(storage.insert(entity, component)?)
            }
            Entry::Vacant(entry) => {
                let mut storage = SparseSet::new();
                let previous = storage.insert(entity, component)?;
                entry.insert(Box::new(storage));
                Ok(previous)
            }
        }
    }

    /// Borrows an entity's component of the requested type.
    ///
    /// Returns `None` if the entity is not live or lacks the component.
    #[must_use]
    pub fn get<T: Send + Sync + 'static>(&self, entity: Entity) -> Option<&T> {
        if !self.is_alive(entity) {
            return None;
        }
        let erased: &dyn Any = self.components.get(&TypeId::of::<T>())?.as_ref();
        erased.downcast_ref::<SparseSet<T>>()?.get(entity)
    }

    /// Mutably borrows an entity's component of the requested type.
    ///
    /// Returns `None` if the entity is not live or lacks the component.
    #[must_use]
    pub fn get_mut<T: Send + Sync + 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.is_alive(entity) {
            return None;
        }
        let erased: &mut dyn Any = self.components.get_mut(&TypeId::of::<T>())?.as_mut();
        erased.downcast_mut::<SparseSet<T>>()?.get_mut(entity)
    }
}
