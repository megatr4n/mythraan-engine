use std::any::{Any, TypeId, type_name};
use std::collections::{BTreeMap, btree_map::Entry};
use std::num::NonZeroU64;

use mythraan_math::transform::{GlobalTransform, Transform};

use crate::component_storage::ComponentStorage;
use crate::hierarchy::Hierarchy;
use crate::{
    ChangeTickError, Children, Entity, EntityAllocError, EntityAllocator, HierarchyError, Parent,
    Query, QueryError, QueryFilter, QueryMut, ReadQuery, Resources, SparseSet, SparseSetError,
    WorldInsertError, WriteQuery,
};

mod membership;
#[cfg(feature = "reflect")]
mod reflection;
mod scope;
mod transforms;
use membership::{MEMBERSHIP_THRESHOLD, Membership};
pub(crate) use scope::{Borrowed, ensure_access};
pub use scope::{ScopedComponents, ScopedResources, SystemScope};
use transforms::TransformPropagation;

/// Owns the entities, components, and shared resources in a simulation.
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
pub struct World {
    entities: EntityAllocator,
    components: BTreeMap<TypeId, Box<dyn ComponentStorage>>,
    membership: Option<Membership>,
    resources: Resources,
    change_tick: NonZeroU64,
    hierarchy: Hierarchy,
    transforms: TransformPropagation,
    #[cfg(feature = "reflect")]
    reflected: reflection::ReflectedComponents,
}

impl Default for World {
    fn default() -> Self {
        Self {
            entities: EntityAllocator::new(),
            components: BTreeMap::new(),
            membership: None,
            resources: Resources::new(),
            change_tick: NonZeroU64::MIN,
            hierarchy: Hierarchy::default(),
            transforms: TransformPropagation::default(),
            #[cfg(feature = "reflect")]
            reflected: reflection::ReflectedComponents::default(),
        }
    }
}

impl World {
    /// Creates an empty world.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current change tick of this world. The first tick is 1.
    #[must_use]
    pub fn change_tick(&self) -> u64 {
        self.change_tick.get()
    }

    /// Starts the next change tick and returns its value.
    ///
    /// Advance between groups of writes that readers need to distinguish.
    ///
    /// # Errors
    /// Returns an error if advancing would overflow the tick counter.
    pub fn advance_change_tick(&mut self) -> Result<u64, ChangeTickError> {
        self.change_tick = self.change_tick.checked_add(1).ok_or(ChangeTickError)?;
        Ok(self.change_tick())
    }

    /// Borrows the shared resources owned by this world.
    #[must_use]
    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Mutably borrows the shared resources owned by this world.
    ///
    /// ```
    /// use mythraan_ecs::prelude::World;
    ///
    /// struct Gravity(f32);
    ///
    /// let mut world = World::new();
    /// world.resources_mut().insert(Gravity(9.81));
    /// assert_eq!(world.resources().get::<Gravity>().map(|gravity| gravity.0), Some(9.81));
    /// ```
    #[must_use]
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    /// Creates an entity with no components.
    ///
    /// # Errors
    ///
    /// Returns an error if the allocator cannot issue another identifier.
    pub fn spawn(&mut self) -> Result<Entity, EntityAllocError> {
        self.entities.spawn()
    }

    /// Removes an entity, its descendants, and their components.
    ///
    /// Returns false if the entity identifier is not live. Cleanup removes its
    /// components before the entity slot can be reused.
    /// Descendants are removed before parents. If a destructor unwinds, its
    /// entity remains live and a retry resumes cleanup of the remaining subtree.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.is_alive(entity) {
            return false;
        }
        let mut leaf = entity;
        loop {
            if let Some(child) = self
                .children(leaf)
                .and_then(|children| children.as_slice().last())
            {
                leaf = *child;
                continue;
            }
            if let Some(membership) = &mut self.membership {
                membership.despawn(&mut self.components, leaf);
            } else {
                for storage in self.components.values_mut() {
                    storage.remove_entity(leaf);
                }
            }
            let parent = self.hierarchy.detach(leaf);
            let removed = self.entities.despawn(leaf);
            if leaf == entity || !removed {
                return removed;
            }
            let Some(parent) = parent else {
                return false;
            };
            leaf = parent;
        }
    }

    /// Borrows the entity's direct parent, if present.
    #[must_use]
    pub fn parent(&self, entity: Entity) -> Option<&Parent> {
        self.hierarchy.parent(entity)
    }

    /// Borrows the entity's direct children, if present.
    #[must_use]
    pub fn children(&self, entity: Entity) -> Option<&Children> {
        self.hierarchy.children(entity)
    }

    /// Assigns a new parent or makes the child a root when given `None`.
    ///
    /// Use this method to update the parent link and both child lists together.
    ///
    /// ```
    /// use mythraan_ecs::prelude::{Parent, World};
    ///
    /// let mut world = World::new();
    /// let root = world.spawn()?;
    /// let child = world.spawn()?;
    /// world.set_parent(child, Some(root))?;
    /// assert_eq!(world.parent(child).map(Parent::entity), Some(root));
    /// assert!(world.despawn(root));
    /// assert!(!world.is_alive(child));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Rejects stale entities, cycles, and failed storage allocations.
    pub fn set_parent(
        &mut self,
        child: Entity,
        parent: Option<Entity>,
    ) -> Result<(), HierarchyError> {
        if !self.is_alive(child) {
            return Err(HierarchyError::EntityNotAlive(child));
        }
        if let Some(parent) = parent
            && !self.is_alive(parent)
        {
            return Err(HierarchyError::EntityNotAlive(parent));
        }
        self.hierarchy.set_parent(child, parent)
    }

    /// Returns whether this entity identifier is live in the world.
    #[must_use]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    /// Iterates over the entities that have every component specified by the query.
    pub fn query<Q: ReadQuery>(&self) -> Query<'_, Q> {
        Query::new(self)
    }

    /// Borrows the world exclusively and prepares component access for a query.
    ///
    /// # Errors
    ///
    /// Returns a conflict if a component is requested more than once and at
    /// least one request is mutable.
    pub fn query_mut<Q: WriteQuery>(&mut self) -> Result<QueryMut<'_, Q>, QueryError> {
        self.query_filtered_mut::<Q, ()>()
    }

    /// Prepares a filtered query. Change filters only match the current tick.
    ///
    /// # Errors
    ///
    /// Returns the same access conflict as [`Self::query_mut`].
    pub fn query_filtered_mut<Q: WriteQuery, F: QueryFilter>(
        &mut self,
    ) -> Result<QueryMut<'_, Q>, QueryError> {
        self.query_filtered_mut_since::<Q, F>(self.change_tick() - 1)
    }

    /// Prepares a filtered query whose change filters match ticks after `after_tick`.
    ///
    /// Use a tick from this world. Pass zero to include every recorded tick.
    ///
    /// # Errors
    ///
    /// Returns the same access conflict as [`Self::query_mut`].
    pub fn query_filtered_mut_since<Q: WriteQuery, F: QueryFilter>(
        &mut self,
        after_tick: u64,
    ) -> Result<QueryMut<'_, Q>, QueryError> {
        let tick = self.change_tick();
        let transforms = &mut self.transforms;
        QueryMut::new(
            &self.entities,
            |bounds| {
                self.components
                    .range_mut(bounds)
                    .map(|(&id, storage)| (id, Borrowed::Exclusive(storage.as_mut())))
            },
            F::requests(),
            after_tick,
            tick,
            |id| transforms.invalidate(id),
        )
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
        let tick = self.change_tick();
        self.transforms.invalidate(TypeId::of::<T>());

        match self.components.entry(TypeId::of::<T>()) {
            Entry::Occupied(mut entry) => {
                let erased: &mut dyn Any = entry.get_mut().as_mut();
                let storage = erased.downcast_mut::<SparseSet<T>>().ok_or(
                    WorldInsertError::StorageTypeMismatch {
                        component: type_name::<T>(),
                    },
                )?;
                if let Some(membership) = &mut self.membership {
                    let row = entity.index() as usize;
                    let id = storage.membership_id;
                    membership
                        .prepare(row, id)
                        .map_err(SparseSetError::Reserve)?;
                    let previous = storage.insert_at_tick(entity, component, tick)?;
                    if previous.is_none() {
                        membership.add(row, id);
                    }
                    Ok(previous)
                } else {
                    Ok(storage.insert_at_tick(entity, component, tick)?)
                }
            }
            Entry::Vacant(_) => self.insert_new_type(entity, component, tick),
        }
    }

    #[cold]
    fn insert_new_type<T: Send + Sync + 'static>(
        &mut self,
        entity: Entity,
        component: T,
        tick: u64,
    ) -> Result<Option<T>, WorldInsertError> {
        let ty = TypeId::of::<T>();
        let mut storage = SparseSet::tracked();
        let previous = storage.insert_at_tick(entity, component, tick)?;
        let row = entity.index() as usize;
        if let Some(membership) = &mut self.membership {
            let id = u32::try_from(membership.next_id()).map_err(|_| {
                SparseSetError::IndexOutOfRange {
                    index: entity.index(),
                }
            })?;
            membership
                .prepare_type(row, id)
                .map_err(SparseSetError::Reserve)?;
            storage.membership_id = id;
            membership.register(ty);
            membership.add(row, id);
        } else if self.components.len() + 1 == MEMBERSHIP_THRESHOLD {
            // Stage the complete index before assigning IDs or publishing the new storage.
            let membership =
                Membership::build(&self.components, ty, entity).map_err(SparseSetError::Reserve)?;
            for (id, existing) in self.components.values_mut().enumerate() {
                existing.set_membership_id(id as u32);
            }
            storage.membership_id = self.components.len() as u32;
            self.membership = Some(membership);
        }
        self.components.insert(ty, Box::new(storage));
        Ok(previous)
    }

    /// Borrows an entity's component of the requested type.
    ///
    /// Returns `None` if the entity is not live or lacks the component.
    #[must_use]
    pub fn get<T: Send + Sync + 'static>(&self, entity: Entity) -> Option<&T> {
        if !self.is_alive(entity) {
            return None;
        }
        self.storage::<T>()?.get(entity)
    }

    pub(crate) fn storage<T: Send + Sync + 'static>(&self) -> Option<&SparseSet<T>> {
        let erased: &dyn Any = self.components.get(&TypeId::of::<T>())?.as_ref();
        erased.downcast_ref()
    }

    /// Mutably borrows an entity's component of the requested type.
    ///
    /// Returns `None` if the entity is not live or lacks the component.
    /// A successful borrow marks the component as changed in the current tick.
    #[must_use]
    pub fn get_mut<T: Send + Sync + 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let tick = self.change_tick();
        self.transforms.invalidate(TypeId::of::<T>());
        self.storage_mut::<T>(entity)?.get_mut_at_tick(entity, tick)
    }

    /// Removes and returns an entity's component of the requested type.
    ///
    /// Returns `None` if the entity is not live or lacks the component.
    /// Removing a `Transform` also clears its derived `GlobalTransform`.
    pub fn remove<T: Send + Sync + 'static>(&mut self, entity: Entity) -> Option<T> {
        if !self.is_alive(entity) {
            return None;
        }
        let removed = self.remove_component::<T>(entity)?;
        let id = TypeId::of::<T>();
        if id == TypeId::of::<Transform>() {
            if self.children(entity).is_some() {
                if self.hierarchy.dirty_subtrees.try_reserve(1).is_ok() {
                    self.hierarchy.dirty_subtrees.insert(entity);
                } else {
                    self.transforms.rebuild = true;
                }
            }
            self.remove_component::<GlobalTransform>(entity);
        } else {
            self.transforms.invalidate(id);
        }
        Some(removed)
    }

    fn remove_component<T: Send + Sync + 'static>(&mut self, entity: Entity) -> Option<T> {
        let erased: &mut dyn Any = self.components.get_mut(&TypeId::of::<T>())?.as_mut();
        let storage = erased.downcast_mut::<SparseSet<T>>()?;
        let removed = storage.remove(entity)?;
        if let Some(membership) = &mut self.membership {
            membership.remove(entity.index() as usize, storage.membership_id);
        }
        Some(removed)
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

#[cfg(test)]
mod membership_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_tick_does_not_wrap_after_exhaustion() {
        let mut world = World {
            change_tick: NonZeroU64::MAX,
            ..World::new()
        };
        assert_eq!(world.advance_change_tick(), Err(ChangeTickError));
        assert_eq!(world.change_tick(), u64::MAX);
    }
}
