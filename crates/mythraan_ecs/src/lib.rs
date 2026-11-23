//! Entities, components, and worlds for Mythraan's entity component system.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod prelude;

mod component_storage;
mod entity;
mod error;
mod query;
mod resources;
mod sparse_set;
mod world;

pub use entity::{Entity, EntityAllocator};
pub use error::{EntityAllocError, SparseSetError, WorldInsertError};
pub use query::{Query, ReadQuery};
pub use resources::Resources;
pub use sparse_set::SparseSet;
pub use world::World;
