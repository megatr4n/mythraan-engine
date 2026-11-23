use std::any::{Any, TypeId, type_name};
use std::collections::{BTreeMap, btree_map::Entry};

use crate::component_storage::ComponentStorage;
use crate::{Entity, EntityAllocError, EntityAllocator, SparseSet, WorldInsertError};

/// Owns the entities and their components in a simulation.
///
/// Use entity identifiers with the world that issued them.
///
/// ```
/// use mythraan_ecs::prelude::World;
///
/// struct Health(u32);
///
/// let mut world = World::new();
/// let entity = world.spawn()?;
/// world.insert(entity, Health(100))?;
/// if let Some(health) = world.get_mut::<Health>(entity) {
///     health.0 -= 10;
/// }
/// assert_eq!(world.get::<Health>(entity).map(|health| health.0), Some(90));
/// assert!(world.despawn(entity));
/// assert!(world.get::<Health>(entity).is_none());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
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

    /// Removes an entity and all of its components.
    ///
    /// Returns false if the entity identifier is not live. Cleanup visits each
    /// registered component storage before the entity slot can be reused.
    /// If a component's destructor unwinds, the entity remains live and calling
    /// this method again cleans up any remaining components.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.is_alive(entity) {
            return false;
        }
        for storage in self.components.values_mut() {
            storage.remove_entity(entity);
        }
        self.entities.despawn(entity)
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
        self.storage_mut::<T>(entity)?.get_mut(entity)
    }

    /// Removes and returns an entity's component of the requested type.
    ///
    /// Returns `None` if the entity is not live or lacks the component.
    pub fn remove<T: Send + Sync + 'static>(&mut self, entity: Entity) -> Option<T> {
        self.storage_mut::<T>(entity)?.remove(entity)
    }

    fn storage_mut<T: Send + Sync + 'static>(
        &mut self,
        entity: Entity,
    ) -> Option<&mut SparseSet<T>> {
        if !self.is_alive(entity) {
            return None;
        }
        let erased: &mut dyn Any = self.components.get_mut(&TypeId::of::<T>())?.as_mut();
        erased.downcast_mut()
    }
}
