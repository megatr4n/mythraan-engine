use std::mem;
use std::num::NonZeroUsize;

use crate::{Entity, SparseSetError};

#[derive(Clone, Copy, Default)]
pub struct SparseIndex<'world> {
    sparse: &'world [Option<NonZeroUsize>],
    entities: &'world [Entity],
}

impl<'world> SparseIndex<'world> {
    pub(crate) fn entities(self) -> &'world [Entity] {
        self.entities
    }

    pub(crate) fn position(self, entity: Entity) -> Option<usize> {
        let sparse_index = usize::try_from(entity.index()).ok()?;
        let index = self.sparse.get(sparse_index)?.as_ref()?.get() - 1;
        (self.entities.get(index) == Some(&entity)).then_some(index)
    }
}

/// Stores one packed component per entity index within a single allocator.
///
/// The caller validates liveness with that allocator before inserting.
/// Lookup and removal require the full stored entity, including its generation.
/// Removal can change iteration order.
///
/// ```
/// use mythraan_ecs::prelude::{EntityAllocator, SparseSet};
///
/// let mut entities = EntityAllocator::new();
/// let mut health = SparseSet::new();
/// let entity = entities.spawn()?;
/// health.insert(entity, 100)?;
/// assert_eq!(health.get(entity), Some(&100));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct SparseSet<T> {
    // Dense positions are one-based so empty entries fit in one word.
    sparse: Vec<Option<NonZeroUsize>>,
    entities: Vec<Entity>,
    values: Vec<T>,
}

impl<T> SparseSet<T> {
    /// Creates an empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sparse: Vec::new(),
            entities: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Returns the number of stored components.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether the set is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Inserts a component and returns the old value if the same entity owned one.
    ///
    /// Growing the sparse table initializes all intervening entries.
    ///
    /// # Errors
    ///
    /// Fails if a different generation owns the index, the index cannot be
    /// addressed, or storage cannot be reserved.
    pub fn insert(&mut self, entity: Entity, value: T) -> Result<Option<T>, SparseSetError> {
        let index = entity.index();
        let sparse_index =
            usize::try_from(index).map_err(|_| SparseSetError::IndexOutOfRange { index })?;
        if let Some(position) = self.sparse.get(sparse_index).copied().flatten() {
            let dense_index = position.get() - 1;
            if self.entities.get(dense_index) == Some(&entity)
                && let Some(previous) = self.values.get_mut(dense_index)
            {
                return Ok(Some(mem::replace(previous, value)));
            }
            return Err(SparseSetError::GenerationMismatch { index });
        }

        let required = sparse_index
            .checked_add(1)
            .ok_or(SparseSetError::IndexOutOfRange { index })?;
        self.sparse
            .try_reserve(required.saturating_sub(self.sparse.len()))?;
        self.entities.try_reserve(1)?;
        self.values.try_reserve(1)?;
        if required > self.sparse.len() {
            self.sparse.resize(required, None);
        }
        let entry = self
            .sparse
            .get_mut(sparse_index)
            .ok_or(SparseSetError::IndexOutOfRange { index })?;
        *entry = NonZeroUsize::new(self.entities.len() + 1);
        self.entities.push(entity);
        self.values.push(value);
        Ok(None)
    }

    /// Returns the component for this entity in O(1) time.
    #[must_use]
    pub fn get(&self, entity: Entity) -> Option<&T> {
        self.values.get(self.dense_index(entity)?)
    }

    /// Returns a mutable reference to the component for this entity in O(1) time.
    #[must_use]
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let index = self.dense_index(entity)?;
        self.values.get_mut(index)
    }

    /// Removes and returns the component for this entity in O(1) time.
    pub fn remove(&mut self, entity: Entity) -> Option<T> {
        let index = self.dense_index(entity)?;
        let sparse_index = usize::try_from(entity.index()).ok()?;
        *self.sparse.get_mut(sparse_index)? = None;

        self.entities.swap_remove(index);
        let value = self.values.swap_remove(index);
        if let Some(moved) = self.entities.get(index)
            && let Some(entry) = self.sparse.get_mut(moved.index() as usize)
        {
            *entry = NonZeroUsize::new(index + 1);
        }
        Some(value)
    }

    /// Returns the packed component values in iteration order.
    #[must_use]
    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub(crate) fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub(crate) fn query_parts(&mut self) -> (SparseIndex<'_>, &mut Vec<T>) {
        let index = SparseIndex {
            sparse: &self.sparse,
            entities: &self.entities,
        };
        (index, &mut self.values)
    }

    /// Iterates over the stored entities and their components.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (Entity, &T)> {
        self.entities.iter().copied().zip(&self.values)
    }

    /// Iterates over the stored entities and mutable references to their components.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (Entity, &mut T)> {
        self.entities.iter().copied().zip(&mut self.values)
    }

    fn dense_index(&self, entity: Entity) -> Option<usize> {
        SparseIndex {
            sparse: &self.sparse,
            entities: &self.entities,
        }
        .position(entity)
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
