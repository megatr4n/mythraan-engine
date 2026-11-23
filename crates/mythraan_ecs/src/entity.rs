use crate::EntityAllocError;

/// Identifies one generation of an entity slot.
///
/// Use an entity with the allocator that issued it. Separate allocators can
/// issue the same index and generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    index: u32,
    generation: u32,
}

impl Entity {
    /// Returns the entity's slot index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Returns the entity's generation within its slot.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Debug)]
struct Slot {
    generation: u32,
    state: SlotState,
}

#[derive(Debug)]
enum SlotState {
    Live,
    Free { next: Option<u32> },
    Retired,
}

impl Slot {
    fn is_live(&self, generation: u32) -> bool {
        self.generation == generation && matches!(self.state, SlotState::Live)
    }
}

/// Allocates entity identifiers and tracks which generations are live.
///
/// ```
/// use mythraan_ecs::prelude::EntityAllocator;
///
/// let mut entities = EntityAllocator::new();
/// let previous = entities.spawn()?;
/// assert!(entities.despawn(previous));
///
/// let replacement = entities.spawn()?;
/// assert_eq!(replacement.index(), previous.index());
/// assert!(!entities.is_alive(previous));
/// assert!(entities.is_alive(replacement));
/// # Ok::<(), mythraan_ecs::EntityAllocError>(())
/// ```
#[derive(Debug, Default)]
pub struct EntityAllocator {
    slots: Vec<Slot>,
    free_head: Option<u32>,
}

impl EntityAllocator {
    /// Creates an empty allocator.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
        }
    }

    /// Allocates an identifier, reusing a free slot when available.
    ///
    /// Reusing a slot takes O(1) time. Growing the allocator is amortized O(1).
    ///
    /// # Errors
    ///
    /// Returns an error when a new slot's index cannot be represented or its
    /// storage cannot be reserved.
    pub fn spawn(&mut self) -> Result<Entity, EntityAllocError> {
        if let Some(index) = self.free_head
            && let Some(slot) = self.slots.get_mut(index as usize)
            && let SlotState::Free { next } = slot.state
        {
            self.free_head = next;
            slot.state = SlotState::Live;
            return Ok(Entity {
                index,
                generation: slot.generation,
            });
        }

        let index =
            u32::try_from(self.slots.len()).map_err(|_| EntityAllocError::IndexExhausted)?;
        self.slots.try_reserve(1)?;
        self.slots.push(Slot {
            generation: 0,
            state: SlotState::Live,
        });
        Ok(Entity {
            index,
            generation: 0,
        })
    }

    /// Releases a live entity's identifier in O(1) time.
    ///
    /// Returns false when the index or generation is stale or unrecognized.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        let Some(slot) = self.slots.get_mut(entity.index as usize) else {
            return false;
        };
        if !slot.is_live(entity.generation) {
            return false;
        }

        slot.state = if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            SlotState::Free {
                next: self.free_head.replace(entity.index),
            }
        } else {
            // Retire the slot before a generation can be issued twice.
            SlotState::Retired
        };
        true
    }

    /// Checks in O(1) time that the entity's index and generation are live.
    #[must_use]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.slots
            .get(entity.index as usize)
            .is_some_and(|slot| slot.is_live(entity.generation))
    }
}
