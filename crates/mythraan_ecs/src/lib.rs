//! Entities, components, and worlds for Mythraan's entity component system.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod prelude;

mod change_detection;
mod command_buffer;
mod component_storage;
mod conflict_graph;
mod entity;
mod error;
mod events;
mod hierarchy;
mod query;
mod query_mut;
mod resources;
mod schedule;
mod sparse_set;
mod system_access;
mod world;

pub use command_buffer::{
    CommandBuffer, CommandFailure, CommandTarget, Commands, DeferredEntity, FlushReport,
};
pub use conflict_graph::ConflictGraph;
pub use entity::{Entity, EntityAllocator};
pub use error::{
    ChangeTickError, CommandError, EntityAllocError, EventReadError, EventWriteError,
    HierarchyError, QueryError, ScheduleError, ScopeError, SparseSetError, StorageKind,
    WorldInsertError,
};
pub use events::{EventReader, EventWriter, Events};
pub use hierarchy::{Children, Parent};
pub use query::{Query, ReadQuery};
pub use query_mut::{
    Added, Changed, FilteredQuery, QueryFilter, QueryMut, With, Without, WriteQuery,
};
pub use resources::{Res, ResMut, Resources};
pub use schedule::{Schedule, SystemFlush};
pub use sparse_set::SparseSet;
pub use system_access::{AccessSet, SystemAccess, SystemParam};
pub use world::{ScopedComponents, ScopedResources, SystemScope, World};
