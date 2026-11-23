use std::mem;

use crate::EventWriteError;

/// A queue that retains events for their frame of submission and the next frame.
///
/// Call [`Self::advance_frame`] once at the frame boundary. Each queue
/// owns its events and can be stored as an ECS resource.
pub struct Events<T> {
    previous: Vec<T>,
    current: Vec<T>,
}

impl<T> Events<T> {
    /// Creates an empty event queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            previous: Vec::new(),
            current: Vec::new(),
        }
    }

    /// Borrows the queue for sending events during the current frame.
    #[must_use]
    pub fn writer(&mut self) -> EventWriter<'_, T> {
        EventWriter { events: self }
    }

    /// Starts the next frame, dropping events from the old previous frame.
    ///
    /// Both buffers retain their allocations for reuse.
    pub fn advance_frame(&mut self) {
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
    /// Returns an error if the queue cannot reserve storage.
    pub fn send(&mut self, event: T) -> Result<(), EventWriteError> {
        self.events.current.try_reserve(1)?;
        self.events.current.push(event);
        Ok(())
    }
}
