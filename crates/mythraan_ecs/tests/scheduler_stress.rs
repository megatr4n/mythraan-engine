//! Repeated conflicting and compatible schedules checked against serial results.

mod common;

use common::scoped_pool;

use mythraan_ecs::prelude::{Commands, Entity, QueryMut, Res, Schedule, SystemAccess, World};

const FRAME_COUNT: u64 = if cfg!(miri) { 10 } else { 64 };
const SYSTEM_PAIRS: usize = if cfg!(miri) { 6 } else { 24 };
const ENTITY_COUNT: usize = if cfg!(miri) { 8 } else { 32 };

#[derive(Debug, PartialEq, Eq)]
struct Lane<const L: u8>(u64);

struct Frame(u64);

#[derive(Debug, PartialEq, Eq)]
struct LastWriter(usize);

fn enabled(frame: u64, actor: usize) -> bool {
    !(frame + actor as u64).is_multiple_of(5)
}

fn next(value: u64, frame: u64, actor: usize) -> u64 {
    value
        .wrapping_mul(1_000_003)
        .wrapping_add(frame * 257 + actor as u64 + 1)
}

fn register_writer<const L: u8>(schedule: &mut Schedule, actor: usize, target: Entity) {
    let index = schedule.add_system(
        SystemAccess::of::<(QueryMut<'_, &mut Lane<L>>, Res<'_, Frame>, Commands<'_>)>(),
        move |mut scope, queue| {
            let frame = scope.resources.get::<Frame>()?.unwrap().0;
            scope
                .components
                .query_mut::<&mut Lane<L>>()?
                .for_each(|_, lane| {
                    lane.0 = next(lane.0, frame, actor);
                });
            queue.unwrap().insert(target, LastWriter(actor));
            Ok(())
        },
    );
    assert_eq!(index, actor);
    schedule
        .run_if(index, move |world| {
            enabled(world.resources().get::<Frame>().unwrap().0, actor)
        })
        .unwrap();
}

fn check_frames() {
    let mut world = World::new();
    let target = world.spawn().unwrap();
    world.resources_mut().insert(Frame(0));
    for _ in 0..ENTITY_COUNT {
        let entity = world.spawn().unwrap();
        world.insert(entity, Lane::<0>(0)).unwrap();
        world.insert(entity, Lane::<1>(0)).unwrap();
    }

    let mut schedule = Schedule::new();
    for pair in 0..SYSTEM_PAIRS {
        register_writer::<0>(&mut schedule, pair * 2, target);
        register_writer::<1>(&mut schedule, pair * 2 + 1, target);
    }

    let mut expected = [0; 2];
    for frame in 0..FRAME_COUNT {
        world.resources_mut().get_mut::<Frame>().unwrap().0 = frame;
        world.advance_change_tick().unwrap();
        let active: Vec<_> = (0..SYSTEM_PAIRS * 2)
            .filter(|&actor| enabled(frame, actor))
            .collect();
        for &actor in &active {
            let lane = &mut expected[actor % 2];
            *lane = next(*lane, frame, actor);
        }

        let reports = schedule.run(&mut world).unwrap();
        assert!(
            reports
                .iter()
                .map(|entry| entry.system)
                .eq(active.iter().copied())
        );
        assert!(
            reports
                .iter()
                .all(|entry| entry.report.failures().is_empty())
        );
        assert_eq!(
            world.get::<LastWriter>(target),
            active.last().map(|&actor| LastWriter(actor)).as_ref()
        );
        let mut rows = 0;
        for (_, (left, right)) in world.query::<(&Lane<0>, &Lane<1>)>() {
            assert_eq!([left.0, right.0], expected, "frame {frame}");
            rows += 1;
        }
        assert_eq!(rows, ENTITY_COUNT);
    }
}

#[test]
fn cached_parallel_batches_match_serial_values_and_command_order() {
    for workers in [2, 4] {
        scoped_pool(workers, check_frames);
    }
}
