//! Slot storage for values addressed by generational handles.

use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::handle::{Handle, HandleRef};

struct Slot<T> {
    generation: u64,
    value: SlotValue<T>,
}

enum SlotValue<T> {
    Occupied(T),
    // Free-list links are one-based so that None fits in one word.
    Vacant(Option<NonZeroUsize>),
}

impl<T> SlotValue<T> {
    fn as_ref(&self) -> Option<&T> {
        match self {
            Self::Occupied(value) => Some(value),
            Self::Vacant(_) => None,
        }
    }

    fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Occupied(value) => Some(value),
            Self::Vacant(_) => None,
        }
    }
}

/// Owns values addressed by handles issued by this arena.
///
/// An arena is `Send` or `Sync` when its stored type is.
///
/// ```compile_fail
/// use mythraan_core::prelude::Arena;
/// fn require_send<T: Send>() {}
/// require_send::<Arena<std::rc::Rc<()>>>();
/// ```
///
/// ```compile_fail
/// use mythraan_core::prelude::Arena;
/// fn require_sync<T: Sync>() {}
/// require_sync::<Arena<std::cell::Cell<u32>>>();
/// ```
pub struct Arena<T> {
    identity: Option<Arc<()>>,
    slots: Vec<Slot<T>>,
    free_head: Option<NonZeroUsize>,
    len: usize,
}

impl<T> Arena<T> {
    /// Creates an empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            identity: None,
            slots: Vec::new(),
            free_head: None,
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
        let identity = self.identity.get_or_insert_with(|| Arc::new(()));
        let handle = if let Some(head) = self.free_head
            && let Some(slot) = self.slots.get_mut(head.get() - 1)
            && let SlotValue::Vacant(next) = slot.value
        {
            self.free_head = next;
            slot.value = SlotValue::Occupied(value);
            Handle::new(identity, head.get() - 1, slot.generation)
        } else {
            let index = self.slots.len();
            self.slots.push(Slot {
                generation: 0,
                value: SlotValue::Occupied(value),
            });
            Handle::new(identity, index, 0)
        };
        self.len += 1;
        handle
    }

    /// Returns the value for a live handle, in O(1) time.
    ///
    /// Returns `None` for a stale handle or a handle from another arena.
    #[must_use]
    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        let slot = self.slots.get(self.owned_index(handle)?)?;
        if slot.generation != handle.generation() {
            return None;
        }
        slot.value.as_ref()
    }

    /// Returns a mutable reference for a live handle, in O(1) time.
    ///
    /// Returns `None` for a stale handle or a handle from another arena.
    #[must_use]
    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        let index = self.owned_index(handle)?;
        let slot = self.slots.get_mut(index)?;
        if slot.generation != handle.generation() {
            return None;
        }
        slot.value.as_mut()
    }

    /// Removes and returns the value for a live handle, in O(1) time.
    ///
    /// Returns `None` for a stale handle or a handle from another arena.
    pub fn remove(&mut self, handle: &Handle<T>) -> Option<T> {
        let index = self.owned_index(handle)?;
        let slot = self.slots.get_mut(index)?;
        if slot.generation != handle.generation() {
            return None;
        }

        let previous = std::mem::replace(&mut slot.value, SlotValue::Vacant(None));
        let SlotValue::Occupied(value) = previous else {
            slot.value = previous;
            return None;
        };
        // An exhausted slot stays vacant so its generations can never repeat.
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            slot.value = SlotValue::Vacant(self.free_head);
            self.free_head = NonZeroUsize::new(index + 1);
        }
        self.len -= 1;
        Some(value)
    }

    /// Iterates over live handles and values in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.iter_borrowed()
            .map(|(handle, value)| (handle.to_owned(), value))
    }

    /// Iterates over live values and borrowed handles in slot order.
    ///
    /// The iterator borrows the arena identity instead of cloning it for each handle.
    pub fn iter_borrowed(&self) -> impl Iterator<Item = (HandleRef<'_, T>, &T)> {
        let identity = self.identity.as_ref();
        self.slots
            .iter()
            .enumerate()
            .filter_map(move |(index, slot)| {
                let value = slot.value.as_ref()?;
                Some((HandleRef::new(identity?, index, slot.generation), value))
            })
    }

    /// Iterates over live handles and mutable values in slot order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle<T>, &mut T)> {
        self.iter_mut_borrowed()
            .map(|(handle, value)| (handle.to_owned(), value))
    }

    /// Iterates over live mutable values and borrowed handles in slot order.
    ///
    /// The iterator borrows the arena identity instead of cloning it for each handle.
    pub fn iter_mut_borrowed(&mut self) -> impl Iterator<Item = (HandleRef<'_, T>, &mut T)> {
        let identity = self.identity.as_ref();
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(move |(index, slot)| {
                let value = slot.value.as_mut()?;
                Some((HandleRef::new(identity?, index, slot.generation), value))
            })
    }

    fn owned_index(&self, handle: &Handle<T>) -> Option<usize> {
        let identity = self.identity.as_ref()?;
        handle.belongs_to(identity).then_some(handle.index())
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
        assert_eq!(arena.get(&first).unwrap().0, "first");
        assert_eq!(arena.get(&second).unwrap().0, "second");
        let invalid = Handle::new(arena.identity.as_ref().unwrap(), usize::MAX, 0);
        assert!(arena.get(&invalid).is_none());
    }

    #[test]
    fn updates_a_value_through_its_handle() {
        let mut arena = Arena::new();
        let handle = arena.insert(String::from("one"));
        arena.get_mut(&handle).unwrap().push_str(" two");

        assert_eq!(arena.get(&handle).map(String::as_str), Some("one two"));
        let invalid = Handle::new(arena.identity.as_ref().unwrap(), handle.index(), 1);
        assert!(arena.get_mut(&invalid).is_none());
    }

    #[test]
    fn rejects_handles_from_another_arena() {
        let mut first = Arena::new();
        let mut second = Arena::new();
        let foreign = first.insert(10);
        let local = second.insert(20);

        assert_eq!(foreign.index(), local.index());
        assert_eq!(foreign.generation(), local.generation());
        assert_eq!(second.get(&foreign), None);
        assert_eq!(second.get_mut(&foreign), None);
        assert_eq!(second.remove(&foreign), None);
        assert_eq!(second.get(&local), Some(&20));
        assert_eq!(second.len(), 1);
    }

    #[test]
    fn rejects_handles_from_a_dropped_arena_without_retaining_its_values() {
        use std::rc::Rc;

        let (old, value) = {
            let mut arena = Arena::new();
            let value = Rc::new(10);
            let weak = Rc::downgrade(&value);
            (arena.insert(value), weak)
        };
        assert!(value.upgrade().is_none());

        let mut replacement = Arena::new();
        let fresh = replacement.insert(Rc::new(20));
        assert_eq!(old.index(), fresh.index());
        assert_eq!(old.generation(), fresh.generation());
        assert_eq!(replacement.remove(&old), None);
        assert_eq!(replacement.get(&fresh).map(|value| **value), Some(20));
    }

    #[test]
    fn reuses_slots_without_reviving_stale_handles() {
        let mut arena = Arena::new();
        let first = arena.insert(String::from("first"));
        let second = arena.insert(String::from("second"));

        assert_eq!(arena.remove(&first).as_deref(), Some("first"));
        assert!(arena.get(&first).is_none());
        assert!(arena.get_mut(&first).is_none());
        assert!(arena.remove(&first).is_none());
        assert_eq!(arena.remove(&second).as_deref(), Some("second"));
        assert!(arena.is_empty());

        let a = arena.insert(String::from("a"));
        let b = arena.insert(String::from("b"));
        assert_eq!(a.index(), second.index());
        assert_eq!(b.index(), first.index());
        assert_ne!(a.generation(), second.generation());
        assert_ne!(b.generation(), first.generation());
        assert!(arena.get(&first).is_none());
        assert!(arena.get_mut(&second).is_none());
        assert!(arena.remove(&first).is_none());
        assert_eq!(arena.get(&a).map(String::as_str), Some("a"));
        assert_eq!(arena.get(&b).map(String::as_str), Some("b"));
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn retires_a_slot_after_its_last_generation() {
        let mut arena = Arena::new();
        let first = arena.insert(1);
        arena.slots[first.index()].generation = u64::MAX - 1;
        let penultimate = Handle::new(
            arena.identity.as_ref().unwrap(),
            first.index(),
            u64::MAX - 1,
        );
        assert_eq!(arena.remove(&penultimate), Some(1));

        let last = arena.insert(2);
        assert_eq!(last.index(), first.index());
        assert_eq!(last.generation(), u64::MAX);
        assert_eq!(arena.remove(&last), Some(2));

        let next = arena.insert(3);
        assert_ne!(next.index(), last.index());
        assert!(arena.get(&first).is_none());
        assert!(arena.get(&last).is_none());
        assert_eq!(arena.get(&next), Some(&3));
    }

    #[test]
    fn iterates_over_live_slots_with_current_handles() {
        let mut arena = Arena::new();
        let first = arena.insert(10);
        let removed = arena.insert(20);
        let last = arena.insert(30);
        assert_eq!(arena.remove(&removed), Some(20));

        let visited: Vec<_> = arena
            .iter_mut()
            .map(|(handle, value)| {
                *value += 1;
                handle
            })
            .collect();
        assert_eq!(visited, [first.clone(), last.clone()]);

        let current = arena.insert(21);
        let live: Vec<_> = arena
            .iter()
            .map(|(handle, value)| (handle, *value))
            .collect();
        assert_eq!(live, [(first, 11), (current.clone(), 21), (last, 31)]);
        assert_ne!(current, removed);
    }

    #[test]
    fn borrows_current_handles_and_skips_vacant_slots() {
        let mut arena = Arena::new();
        assert!(arena.iter_borrowed().next().is_none());
        let first = arena.insert(10);
        let second = arena.insert(20);
        assert_eq!(arena.remove(&first), Some(10));
        let reused = arena.insert(30);
        assert_eq!(arena.remove(&second), Some(20));

        let owned = {
            let mut entries = arena.iter_borrowed();
            let (handle, value) = entries.next().unwrap();
            assert_eq!(handle.index(), first.index());
            assert_eq!(handle.generation(), reused.generation());
            assert_eq!(*value, 30);
            assert!(entries.next().is_none());
            handle.to_owned()
        };

        assert_eq!(owned, reused);
        assert_eq!(arena.remove(&owned), Some(30));
    }

    #[test]
    fn borrowed_iteration_updates_values_and_can_keep_selected_handles() {
        let mut arena = Arena::new();
        let first = arena.insert(10);
        let removed = arena.insert(20);
        let last = arena.insert(30);
        assert_eq!(arena.remove(&removed), Some(20));

        let mut retained = None;
        for (handle, value) in arena.iter_mut_borrowed() {
            *value += 1;
            if handle.index() == last.index() {
                retained = Some(handle.to_owned());
            }
        }

        let retained = retained.unwrap();
        assert_eq!(retained, last);
        assert_eq!(arena.get(&first), Some(&11));
        assert_eq!(arena.get(&retained), Some(&31));
        assert_eq!(arena.get(&removed), None);
    }

    #[test]
    fn thread_safety_follows_the_stored_type() {
        fn require_send<T: Send>() {}
        fn require_send_sync<T: Send + Sync>() {}

        require_send::<Arena<std::cell::Cell<u32>>>();
        require_send_sync::<Arena<String>>();
        require_send_sync::<Handle<std::rc::Rc<()>>>();
    }
}
