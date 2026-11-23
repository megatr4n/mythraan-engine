//! Conflict graph behavior through the public ECS API.

use mythraan_ecs::AccessSet;
use mythraan_ecs::prelude::{
    Changed, Commands, ConflictGraph, FilteredQuery, Query, QueryMut, Res, ResMut, SystemAccess,
    With,
};

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

fn overlaps(first: &AccessSet, second: &AccessSet) -> bool {
    !first.writes().is_disjoint(second.writes())
        || !first.writes().is_disjoint(second.reads())
        || !first.reads().is_disjoint(second.writes())
}

fn conflicts(first: &SystemAccess, second: &SystemAccess) -> bool {
    overlaps(first.components(), second.components())
        || overlaps(first.resources(), second.resources())
}

#[test]
fn sparse_dependencies_match_pairwise_conflicts_and_batch_levels() {
    const SYSTEMS: usize = 4;
    let options = [
        SystemAccess::of::<()>(),
        SystemAccess::of::<Query<'_, &u32>>(),
        SystemAccess::of::<QueryMut<'_, &mut u32>>(),
        SystemAccess::of::<Res<'_, u32>>(),
        SystemAccess::of::<ResMut<'_, u32>>(),
        SystemAccess::of::<(Query<'_, &u32>, ResMut<'_, u32>)>(),
        SystemAccess::of::<(QueryMut<'_, &mut u32>, Res<'_, u32>)>(),
        SystemAccess::of::<QueryMut<'_, (&mut bool, &u32)>>(),
    ];

    for mut combination in 0..options.len().pow(SYSTEMS as u32) {
        let systems: [SystemAccess; SYSTEMS] = std::array::from_fn(|_| {
            let id = combination % options.len();
            combination /= options.len();
            options[id].clone()
        });
        let graph = ConflictGraph::new(&systems);
        let mut ordered = [[false; SYSTEMS]; SYSTEMS];
        let mut expected_levels = [0; SYSTEMS];
        for (system, access) in systems.iter().enumerate() {
            let mut ancestors = [false; SYSTEMS];
            let dependencies = graph.predecessors(system).unwrap();
            assert!(dependencies.windows(2).all(|pair| pair[0] < pair[1]));
            for &dependency in dependencies {
                assert!(dependency < system);
                assert!(conflicts(&systems[dependency], access));
                ancestors[dependency] = true;
                for (target, inherited) in ancestors.iter_mut().zip(ordered[dependency]) {
                    *target |= inherited;
                }
            }

            ordered[system] = ancestors;
            for (earlier, earlier_access) in systems.iter().take(system).enumerate() {
                if conflicts(earlier_access, access) {
                    assert!(ancestors[earlier]);
                    expected_levels[system] =
                        expected_levels[system].max(expected_levels[earlier] + 1);
                }
            }
        }

        let mut actual_levels = [usize::MAX; SYSTEMS];
        for (level, batch) in graph.batches().enumerate() {
            assert!(batch.windows(2).all(|pair| pair[0] < pair[1]));
            for &system in batch {
                assert_eq!(actual_levels[system], usize::MAX);
                actual_levels[system] = level;
            }
        }
        assert_eq!(actual_levels, expected_levels);
    }
}

#[test]
fn filtered_reads_order_writes_while_deferred_commands_share_the_batch() {
    type Filtered<'world> = FilteredQuery<'world, &'world i8, (With<u16>, Changed<u32>)>;
    let systems = [
        SystemAccess::of::<QueryMut<'_, &mut u32>>(),
        SystemAccess::of::<ResMut<'_, u32>>(),
        SystemAccess::of::<Commands<'_>>(),
        SystemAccess::of::<Filtered<'_>>(),
        SystemAccess::of::<QueryMut<'_, &mut u16>>(),
        SystemAccess::of::<Commands<'_>>(),
    ];
    let graph = ConflictGraph::new(&systems);
    assert_eq!(graph.predecessors(3), Some([0].as_slice()));
    assert_eq!(graph.predecessors(4), Some([3].as_slice()));
    assert_eq!(
        graph.batches().collect::<Vec<_>>(),
        [&[0, 1, 2, 5][..], &[3], &[4]]
    );
    assert_eq!(graph, ConflictGraph::new(&systems));
}

#[test]
fn a_long_write_chain_keeps_one_edge_per_successor() {
    const SYSTEMS: usize = 4_096;
    let write = SystemAccess::of::<QueryMut<'_, &mut u32>>();
    let graph = ConflictGraph::new(&vec![write; SYSTEMS]);

    assert_eq!(graph.batches().len(), SYSTEMS);
    assert!(
        graph
            .batches()
            .enumerate()
            .all(|(system, batch)| batch == [system])
    );
    assert_eq!(graph.predecessors(0), Some([].as_slice()));
    for system in 1..SYSTEMS {
        assert_eq!(graph.predecessors(system), Some([system - 1].as_slice()));
    }
}

#[test]
fn a_writer_waits_for_every_reader_since_the_previous_write() {
    const READERS: usize = 4_096;
    let write = SystemAccess::of::<ResMut<'_, u32>>();
    let read = SystemAccess::of::<Res<'_, u32>>();
    let mut systems = Vec::with_capacity(READERS + 2);
    systems.push(write.clone());
    systems.resize(READERS + 1, read);
    systems.push(write);

    let graph = ConflictGraph::new(&systems);
    let batches: Vec<_> = graph.batches().collect();
    assert_eq!(batches.len(), 3);
    assert_eq!(batches[0], [0]);
    assert!(batches[1].iter().copied().eq(1..=READERS));
    assert_eq!(batches[2], [READERS + 1]);
    assert!(
        graph
            .predecessors(READERS + 1)
            .unwrap()
            .iter()
            .copied()
            .eq(1..=READERS)
    );
}
