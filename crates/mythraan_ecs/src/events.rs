use std::iter::{Chain, FusedIterator};
use std::marker::PhantomData;
use std::sync::Arc;
use std::{mem, slice};

use crate::{EventReadError, EventWriteError};

/// A queue that retains events for their frame of submission and the next frame.
///
/// Call [`Self::advance_frame`] once at the frame boundary. Each queue
/// owns its events and can be stored as an ECS resource.
pub struct Events<T> {
    previous: Vec<T>,
    current: Vec<T>,
    start: u64,
    end: u64,
    identity: Arc<()>,
}

impl<T> Events<T> {
    /// Creates an empty event queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            previous: Vec::new(),
            current: Vec::new(),
            start: 0,
            end: 0,
            identity: Arc::new(()),
        }
    }

    /// Borrows the queue for sending events during the current frame.
    #[must_use]
    pub fn writer(&mut self) -> EventWriter<'_, T> {
        EventWriter { events: self }
    }

    /// Creates an independent reader starting at the oldest retained event.
    #[must_use]
    pub fn reader(&self) -> EventReader<T> {
        EventReader {
            next: self.start,
            identity: Arc::clone(&self.identity),
            marker: PhantomData,
        }
    }

    /// Starts the next frame, dropping events from the old previous frame.
    ///
    /// Both buffers retain their allocations for reuse.
    pub fn advance_frame(&mut self) {
        self.start += self.previous.len() as u64;
        mem::swap(&mut self.previous, &mut self.current);
        self.current.clear();
    }
}

impl<T> Default for Events<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Sends events to the current frame of a queue.
pub struct EventWriter<'queue, T> {
    events: &'queue mut Events<T>,
}

impl<T> EventWriter<'_, T> {
    /// Moves an event into the queue.
    ///
    /// # Errors
    /// Returns an error if the queue cannot reserve storage or its sequence is exhausted.
    pub fn send(&mut self, event: T) -> Result<(), EventWriteError> {
        let end = self
            .events
            .end
            .checked_add(1)
            .ok_or(EventWriteError::SequenceExhausted)?;
        self.events.current.try_reserve(1)?;
        self.events.current.push(event);
        self.events.end = end;
        Ok(())
    }
}

/// Tracks which events one consumer has read from a particular queue.
///
/// Keep a reader between frames. Each reader advances independently.
pub struct EventReader<T> {
    next: u64,
    identity: Arc<()>,
    marker: PhantomData<fn() -> T>,
}

impl<T> EventReader<T> {
    /// Iterates over unread events in submission order.
    ///
    /// Expired events are skipped. Dropping the iterator leaves events it
    /// has not reached unread.
    ///
    /// # Errors
    /// Returns an error if this reader belongs to a different queue.
    pub fn read<'read>(
        &'read mut self,
        events: &'read Events<T>,
    ) -> Result<impl FusedIterator<Item = &'read T>, EventReadError> {
        if !Arc::ptr_eq(&self.identity, &events.identity) {
            return Err(EventReadError);
        }

        self.next = self.next.max(events.start);
        let offset = self.next - events.start;
        let previous_len = events.previous.len() as u64;
        let skip_previous = offset.min(previous_len) as usize;
        let skip_current = offset.saturating_sub(previous_len) as usize;
        let previous = events.previous.get(skip_previous..).unwrap_or_default();
        let current = events.current.get(skip_current..).unwrap_or_default();

        Ok(UnreadEvents {
            next: &mut self.next,
            remaining: previous.iter().chain(current),
        })
    }
}

struct UnreadEvents<'read, T> {
    next: &'read mut u64,
    remaining: Chain<slice::Iter<'read, T>, slice::Iter<'read, T>>,
}

impl<'read, T> Iterator for UnreadEvents<'read, T> {
    type Item = &'read T;

    fn next(&mut self) -> Option<Self::Item> {
        let event = self.remaining.next()?;
        *self.next += 1;
        Some(event)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining.size_hint()
    }
}

impl<T> FusedIterator for UnreadEvents<'_, T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_sequence_preserves_the_last_event_and_existing_readers() {
        let mut events = Events {
            start: u64::MAX - 1,
            end: u64::MAX - 1,
            ..Events::new()
        };
        let mut reader = events.reader();
        events.writer().send(41).unwrap();
        assert!(matches!(
            events.writer().send(42),
            Err(EventWriteError::SequenceExhausted)
        ));

        events.advance_frame();
        assert_eq!(reader.read(&events).unwrap().next(), Some(&41));
        assert_eq!(reader.read(&events).unwrap().next(), None);
        events.advance_frame();
        assert_eq!(reader.read(&events).unwrap().next(), None);
        assert!(matches!(
            events.writer().send(43),
            Err(EventWriteError::SequenceExhausted)
        ));
    }
}
