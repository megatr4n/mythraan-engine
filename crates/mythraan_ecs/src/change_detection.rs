use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub(crate) struct ComponentTicks {
    pub(crate) added: u64,
    // Filters and mutable fetches share this stamp. The world borrow protects the value.
    changed: AtomicU64,
}

impl ComponentTicks {
    pub(crate) fn new(tick: u64) -> Self {
        Self {
            added: tick,
            changed: AtomicU64::new(tick),
        }
    }

    pub(crate) fn changed(&self) -> u64 {
        self.changed.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_changed(&self, tick: u64) {
        self.changed.store(tick, Ordering::Relaxed);
    }
}
