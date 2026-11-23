//! Entity identifiers and allocation for Mythraan's entity component system.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod prelude;

mod entity;
mod error;

pub use entity::{Entity, EntityAllocator};
pub use error::EntityAllocError;
