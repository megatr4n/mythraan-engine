//! Typed generational handles.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Identifies one generation of a slot in the arena that issued it.
///
/// Handles are `Send` and `Sync` for any `T`.
pub struct Handle<T> {
    index: usize,
    generation: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub(crate) const fn new(index: usize, generation: u64) -> Self {
        Self {
            index,
            generation,
            marker: PhantomData,
        }
    }

    /// Returns the slot index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// Returns the generation of the slot when this handle was issued.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
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
    fn handle_identity_uses_index_and_generation_without_value_bounds() {
        struct NotComparable;
        let handle = Handle::<NotComparable>::new(3, 5);
        let copied = handle;
        let keys = std::collections::HashSet::from([handle, Handle::new(3, 6), Handle::new(4, 5)]);

        assert_eq!(copied.index(), 3);
        assert_eq!(copied.generation(), 5);
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&copied));
        assert!(!keys.contains(&Handle::new(4, 6)));
    }
}
