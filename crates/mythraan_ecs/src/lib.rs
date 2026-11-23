//! Entities, components, and worlds for Mythraan's entity component system.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod prelude;

mod command_buffer;
mod component_storage;
mod entity;
mod error;
mod events;
mod query;
mod query_mut;
mod resources;
mod sparse_set;
mod world;

pub use command_buffer::{
    CommandBuffer, CommandFailure, CommandTarget, DeferredEntity, FlushReport,
};
pub use entity::{Entity, EntityAllocator};
pub use error::{
    CommandError, EntityAllocError, EventWriteError, QueryError, SparseSetError, WorldInsertError,
};
pub use events::{EventWriter, Events};
pub use query::{Query, ReadQuery};
pub use query_mut::{QueryFilter, QueryMut, With, Without, WriteQuery};
pub use resources::Resources;
pub use sparse_set::SparseSet;
pub use world::World;
