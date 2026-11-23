//! Conflict graph behavior through the public ECS API.

use mythraan_ecs::prelude::{ConflictGraph, Query, QueryMut, ResMut, SystemAccess};

#[test]
fn orders_writes_and_read_groups_for_a_component() {
    let read = SystemAccess::of::<Query<'_, &u32>>();
    let write = SystemAccess::of::<QueryMut<'_, &mut u32>>();
    let graph = ConflictGraph::new(&[write.clone(), read.clone(), read, write.clone(), write]);

    assert_eq!(graph.predecessors(0), Some([].as_slice()));
    assert_eq!(graph.predecessors(1), Some([0].as_slice()));
    assert_eq!(graph.predecessors(2), Some([0].as_slice()));
    assert_eq!(graph.predecessors(3), Some([1, 2].as_slice()));
    assert_eq!(graph.predecessors(4), Some([3].as_slice()));
    assert_eq!(graph.predecessors(5), None);
}

#[test]
fn deduplicates_edges_across_storage_types() {
    type Both<'world> = (QueryMut<'world, &'world mut u32>, ResMut<'world, u32>);
    let write = SystemAccess::of::<Both<'_>>();
    let graph = ConflictGraph::new(&[write.clone(), write]);

    assert_eq!(graph.predecessors(1), Some([0].as_slice()));
}
