use std::collections::TryReserveError;

use thiserror::Error;

use crate::Entity;

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

/// A component could not be inserted into a sparse set.
#[derive(Debug, Error)]
pub enum SparseSetError {
    /// The table cannot address this entity index on the current platform.
    #[error("component storage cannot address entity index {index}")]
    IndexOutOfRange {
        /// The entity's index.
        index: u32,
    },
    /// A different generation already occupies this entity index.
    #[error("component storage already holds a different generation for entity index {index}")]
    GenerationMismatch {
        /// The occupied entity index.
        index: u32,
    },
    /// Memory could not be reserved for the component or its indices.
    #[error("could not reserve memory for component storage")]
    Reserve(#[from] TryReserveError),
}

/// A component could not be inserted into a world.
#[derive(Debug, Error)]
pub enum WorldInsertError {
    /// The entity identifier is not live in this world.
    #[error("entity {entity:?} is not live in this world")]
    EntityNotAlive {
        /// The rejected identifier.
        entity: Entity,
    },
    /// The stored representation does not match the component type key.
    #[error("component storage does not match the registered type {component}")]
    StorageTypeMismatch {
        /// The requested component type name.
        component: &'static str,
    },
    /// Insertion into the underlying sparse set failed.
    #[error(transparent)]
    Storage(#[from] SparseSetError),
}
