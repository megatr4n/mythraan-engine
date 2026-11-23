//! Conflict graph behavior through the public ECS API.

use mythraan_ecs::prelude::{ConflictGraph, Query, QueryMut, Res, ResMut, SystemAccess};

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

#[test]
fn batches_place_each_system_in_its_earliest_ready_group() {
    type ReadComponentWriteResource<'world> = (Query<'world, &'world u32>, ResMut<'world, bool>);
    type ReadResourceWriteComponent<'world> =
        (Res<'world, bool>, QueryMut<'world, &'world mut i32>);

    let graph = ConflictGraph::new(&[
        SystemAccess::of::<QueryMut<'_, &mut u32>>(),
        SystemAccess::of::<ReadComponentWriteResource<'_>>(),
        SystemAccess::of::<ReadResourceWriteComponent<'_>>(),
        SystemAccess::of::<ResMut<'_, u8>>(),
        SystemAccess::of::<Query<'_, &i32>>(),
        SystemAccess::of::<()>(),
    ]);

    assert_eq!(
        graph.batches().collect::<Vec<_>>(),
        [&[0, 3, 5][..], &[1], &[2], &[4]]
    );
}

#[test]
fn empty_graphs_and_shared_readers_need_no_separation() {
    let empty = ConflictGraph::new(&[]);
    assert_eq!(empty.batches().len(), 0);
    assert_eq!(empty.predecessors(0), None);

    let read = SystemAccess::of::<Res<'_, u32>>();
    let graph = ConflictGraph::new(&[read.clone(), read.clone(), read]);
    assert_eq!(graph.batches().collect::<Vec<_>>(), [&[0, 1, 2][..]]);
}
