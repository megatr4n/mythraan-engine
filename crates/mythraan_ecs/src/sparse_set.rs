use std::num::NonZeroU64;
use std::{fmt, mem};

use crate::change_detection::ComponentTicks;
use crate::{Entity, SparseSetError};

// The low 32 bits store the dense position. The allocator only issues
// nonzero generations for the high 32 bits, leaving the zero word empty.
type SparseSlot = Option<NonZeroU64>;

fn pack_slot(position: u32, entity: Entity) -> SparseSlot {
    NonZeroU64::new((u64::from(entity.generation()) << u32::BITS) | u64::from(position))
}

fn slot_generation(word: u64) -> u32 {
    (word >> u32::BITS) as u32
}

#[derive(Clone, Copy, Default)]
pub struct SparseIndex<'world> {
    sparse: &'world [SparseSlot],
    entities: &'world [Entity],
}

impl<'world> SparseIndex<'world> {
    pub(crate) fn entities(self) -> &'world [Entity] {
        self.entities
    }

    pub(crate) fn position(self, entity: Entity) -> Option<usize> {
        let sparse_index = usize::try_from(entity.index()).ok()?;
        let word = self.sparse.get(sparse_index)?.as_ref()?.get();
        (slot_generation(word) == entity.generation()).then_some(word as u32 as usize)
    }

    pub(crate) fn contains(self, entity: Entity) -> bool {
        let Ok(index) = usize::try_from(entity.index()) else {
            return false;
        };
        self.sparse.get(index).is_some_and(|slot| {
            // Empty slots report generation zero. No entity has that generation.
            slot_generation(slot.map_or(0, NonZeroU64::get)) == entity.generation()
        })
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
pub struct SparseSet<T> {
    pub(crate) membership_id: u32,
    sparse: Vec<SparseSlot>,
    entities: Vec<Entity>,
    values: Vec<T>,
    ticks: Option<Vec<ComponentTicks>>,
}

struct DebugSlots<'a>(&'a [SparseSlot]);

impl fmt::Debug for DebugSlots<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for slot in self.0 {
            match slot {
                Some(word) => list.entry(&(word.get() as u32, slot_generation(word.get()))),
                None => list.entry(&None::<()>),
            };
        }
        list.finish()
    }
}

impl<T: fmt::Debug> fmt::Debug for SparseSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SparseSet")
            .field("sparse", &DebugSlots(&self.sparse))
            .field("entities", &self.entities)
            .field("values", &self.values)
            .field("ticks", &self.ticks)
            .finish()
    }
}

impl<T> SparseSet<T> {
    /// Creates an empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            membership_id: 0,
            sparse: Vec::new(),
            entities: Vec::new(),
            values: Vec::new(),
            ticks: None,
        }
    }

    pub(crate) fn tracked() -> Self {
        Self {
            ticks: Some(Vec::new()),
            ..Self::new()
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
    /// Growing the sparse table initializes all intervening entries. The first
    /// sparse allocation uses the standard allocation failure handler.
    ///
    /// # Errors
    ///
    /// Fails if a different generation owns the index, the index cannot be
    /// addressed, or storage cannot be reserved.
    pub fn insert(&mut self, entity: Entity, value: T) -> Result<Option<T>, SparseSetError> {
        self.insert_at_tick(entity, value, 0)
    }

    pub(crate) fn insert_at_tick(
        &mut self,
        entity: Entity,
        value: T,
        tick: u64,
    ) -> Result<Option<T>, SparseSetError> {
        let index = entity.index();
        let sparse_index =
            usize::try_from(index).map_err(|_| SparseSetError::IndexOutOfRange { index })?;
        if let Some(slot) = self.sparse.get(sparse_index).copied().flatten() {
            let word = slot.get();
            let dense_index = word as u32 as usize;
            if slot_generation(word) == entity.generation()
                && let Some(previous) = self.values.get_mut(dense_index)
            {
                if let Some(ticks) = self.ticks.as_ref().and_then(|ticks| ticks.get(dense_index)) {
                    ticks.mark_changed(tick);
                }
                return Ok(Some(mem::replace(previous, value)));
            }
            return Err(SparseSetError::GenerationMismatch { index });
        }

        let required = sparse_index
            .checked_add(1)
            .ok_or(SparseSetError::IndexOutOfRange { index })?;
        let position = u32::try_from(self.entities.len())
            .map_err(|_| SparseSetError::IndexOutOfRange { index })?;
        self.entities.try_reserve(1)?;
        self.values.try_reserve(1)?;
        if let Some(ticks) = &mut self.ticks {
            ticks.try_reserve(1)?;
        }
        if required > self.sparse.len() {
            if self.sparse.is_empty() && required <= isize::MAX as usize / size_of::<SparseSlot>() {
                // Repeating the zero niche lets Vec request zeroed memory.
                // Start with room for at least four slots, matching Vec growth.
                self.sparse = vec![None; required.max(4)];
            } else {
                self.sparse.try_reserve(required - self.sparse.len())?;
                self.sparse.resize(required, None);
            }
        }
        let entry = self
            .sparse
            .get_mut(sparse_index)
            .ok_or(SparseSetError::IndexOutOfRange { index })?;
        *entry = pack_slot(position, entity);
        self.entities.push(entity);
        self.values.push(value);
        if let Some(ticks) = &mut self.ticks {
            ticks.push(ComponentTicks::new(tick));
        }
        Ok(None)
    }

    /// Returns the component for this entity in O(1) time.
    #[must_use]
    pub fn get(&self, entity: Entity) -> Option<&T> {
        self.values.get(self.dense_index(entity)?)
    }

    pub(crate) fn get_with_change_tick(&self, entity: Entity) -> Option<(&T, u64)> {
        let index = self.dense_index(entity)?;
        let ticks = self.ticks.as_ref()?.get(index)?;
        Some((self.values.get(index)?, ticks.changed()))
    }

    /// Returns a mutable reference to the component for this entity in O(1) time.
    #[must_use]
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let index = self.dense_index(entity)?;
        self.values.get_mut(index)
    }

    pub(crate) fn get_mut_at_tick(&mut self, entity: Entity, tick: u64) -> Option<&mut T> {
        let (value, ticks) = self.get_mut_with_ticks(entity)?;
        if let Some(ticks) = ticks {
            ticks.mark_changed(tick);
        }
        Some(value)
    }

    pub(crate) fn get_mut_with_ticks(
        &mut self,
        entity: Entity,
    ) -> Option<(&mut T, Option<&ComponentTicks>)> {
        let index = self.dense_index(entity)?;
        let value = self.values.get_mut(index)?;
        let ticks = self.ticks.as_ref().and_then(|ticks| ticks.get(index));
        Some((value, ticks))
    }

    /// Removes and returns the component for this entity in O(1) time.
    pub fn remove(&mut self, entity: Entity) -> Option<T> {
        let index = self.dense_index(entity)?;
        let sparse_index = usize::try_from(entity.index()).ok()?;
        *self.sparse.get_mut(sparse_index)? = None;

        self.entities.swap_remove(index);
        let value = self.values.swap_remove(index);
        if let Some(ticks) = &mut self.ticks {
            ticks.swap_remove(index);
        }
        if let Some(moved) = self.entities.get(index)
            && let Some(entry) = self.sparse.get_mut(moved.index() as usize)
        {
            // The removed position was decoded from the low 32 bits of its slot.
            *entry = pack_slot(index as u32, *moved);
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

    pub(crate) fn query_parts_ref(&self) -> (SparseIndex<'_>, &[ComponentTicks], &Vec<T>) {
        let index = SparseIndex {
            sparse: &self.sparse,
            entities: &self.entities,
        };
        (
            index,
            self.ticks.as_deref().unwrap_or_default(),
            &self.values,
        )
    }

    pub(crate) fn query_parts(&mut self) -> (SparseIndex<'_>, &[ComponentTicks], &mut Vec<T>) {
        let index = SparseIndex {
            sparse: &self.sparse,
            entities: &self.entities,
        };
        (
            index,
            self.ticks.as_deref().unwrap_or_default(),
            &mut self.values,
        )
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

#[cfg(test)]
mod tests {
    use super::{SparseSet, SparseSlot, pack_slot, slot_generation};
    use crate::EntityAllocator;

    #[test]
    fn debug_decodes_positions_and_generations_after_reuse() {
        let mut allocator = EntityAllocator::new();
        let first = allocator.spawn().unwrap();
        let second = allocator.spawn().unwrap();
        let mut storage = SparseSet::new();
        storage.insert(first, 10).unwrap();
        storage.insert(second, 20).unwrap();
        storage.remove(first);
        assert!(allocator.despawn(first));
        let replacement = allocator.spawn().unwrap();
        storage.insert(replacement, 30).unwrap();
        assert!(format!("{storage:?}").contains("sparse: [(1, 2), (0, 1), None, None]"));
    }

    #[test]
    fn one_word_encodes_both_dense_position_boundaries() {
        assert_eq!(size_of::<SparseSlot>(), size_of::<u64>());
        let entity = EntityAllocator::new().spawn().unwrap();
        for position in [0, u32::MAX] {
            let word = pack_slot(position, entity).unwrap().get();
            assert_eq!(word as u32, position);
            assert_eq!(slot_generation(word), entity.generation());
        }
    }

    #[test]
    fn membership_rejects_holes_out_of_range_and_stale_generations() {
        let mut entities = EntityAllocator::new();
        let stale = entities.spawn().unwrap();
        let hole = entities.spawn().unwrap();
        let last = entities.spawn().unwrap();
        let outside = entities.spawn().unwrap();
        let mut components = SparseSet::new();
        components.insert(last, 5).unwrap();
        assert!(entities.despawn(stale));
        let live = entities.spawn().unwrap();
        assert_eq!(live.index(), stale.index());
        components.insert(live, 8).unwrap();

        let (index, _, _) = components.query_parts_ref();
        assert!(index.contains(last));
        assert!(index.contains(live));
        assert!(!index.contains(stale));
        assert!(!index.contains(hole));
        assert!(!index.contains(outside));
    }
}
