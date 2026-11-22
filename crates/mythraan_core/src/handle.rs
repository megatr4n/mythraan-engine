//! Typed generational handles.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

/// Identifies one generation of a slot in the arena that issued it.
///
/// Handles are `Send` and `Sync` for any `T`.
pub struct Handle<T> {
    // Keep the arena's identity alive while any issued handle exists.
    arena: Arc<()>,
    index: usize,
    generation: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub(crate) fn new(arena: &Arc<()>, index: usize, generation: u64) -> Self {
        Self {
            arena: Arc::clone(arena),
            index,
            generation,
            marker: PhantomData,
        }
    }

    pub(crate) fn belongs_to(&self, arena: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.arena, arena)
    }

    /// Returns the slot index.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the generation of the slot when this handle was issued.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self::new(&self.arena, self.index, self.generation)
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.generation == other.generation
            && self.belongs_to(&other.arena)
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.arena), state);
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Handle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_identity_uses_arena_index_and_generation_without_value_bounds() {
        struct NotComparable;
        let arena = Arc::new(());
        let other_arena = Arc::new(());
        let handle = Handle::<NotComparable>::new(&arena, 3, 5);
        let cloned = handle.clone();
        let keys = std::collections::HashSet::from([
            handle,
            Handle::new(&arena, 3, 6),
            Handle::new(&arena, 4, 5),
            Handle::new(&other_arena, 3, 5),
        ]);

        assert_eq!(cloned.index(), 3);
        assert_eq!(cloned.generation(), 5);
        assert_eq!(keys.len(), 4);
        assert!(keys.contains(&cloned));
        assert!(!keys.contains(&Handle::new(&arena, 4, 6)));
    }
}
