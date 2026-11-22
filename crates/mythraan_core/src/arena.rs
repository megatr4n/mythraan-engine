//! Slot storage for values addressed by generational handles.

use crate::handle::Handle;

struct Slot<T> {
    generation: u64,
    value: Option<T>,
}

/// Owns values addressed by handles issued by this arena.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    len: usize,
}

impl<T> Arena<T> {
    /// Creates an empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            len: 0,
        }
    }

    /// Returns the number of live values.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the arena contains no live values.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Stores a value and returns its handle. Takes amortized O(1) time.
    #[must_use]
    pub fn insert(&mut self, value: T) -> Handle<T> {
        let index = self.slots.len();
        self.slots.push(Slot {
            generation: 0,
            value: Some(value),
        });
        self.len += 1;
        Handle::new(index, 0)
    }

    /// Returns the value for a live handle, in O(1) time.
    #[must_use]
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        let slot = self.slots.get(handle.index())?;
        if slot.generation != handle.generation() {
            return None;
        }
        slot.value.as_ref()
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_and_reads_owned_values() {
        struct Value(String);
        let mut arena = Arena::default();
        assert!(arena.is_empty());

        let first = arena.insert(Value("first".into()));
        let second = arena.insert(Value("second".into()));
        assert_ne!(first.index(), second.index());
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.get(first).unwrap().0, "first");
        assert_eq!(arena.get(second).unwrap().0, "second");
        assert!(arena.get(Handle::new(usize::MAX, 0)).is_none());
    }
}
