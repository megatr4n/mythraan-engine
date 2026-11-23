use std::collections::TryReserveError;

use thiserror::Error;

/// An allocator could not issue an entity identifier.
#[derive(Debug, Error)]
pub enum EntityAllocError {
    /// All representable slot indices are in use or retired.
    #[error("entity index space exhausted")]
    IndexExhausted,
    /// Memory could not be reserved for a new slot.
    #[error("could not reserve memory for an entity slot")]
    Reserve(#[from] TryReserveError),
}
