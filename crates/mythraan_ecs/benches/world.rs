//! Measures ECS queries, deferred mutations, and cached schedule execution.

use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mythraan_ecs::prelude::{CommandBuffer, Schedule, SystemAccess, With, Without, World};

const ENTITY_COUNT: u64 = 100_000;
const COMMAND_ENTITIES: u64 = 1_024;

struct Position(u64);
struct Velocity(u64);
struct Moving;
struct Disabled;

fn populated_world() -> World {
    let mut world = World::new();
    let entities = (0..ENTITY_COUNT)
        .map(|index| {
            let entity = world.spawn().unwrap();
            world.insert(entity, Position(index)).unwrap();
            if index % 2 == 0 {
                world.insert(entity, Moving).unwrap();
            }
            if index % 4 == 0 {
                world.insert(entity, Disabled).unwrap();
            }
            (entity, index)
        })
        .collect::<Vec<_>>();

    for (entity, index) in entities.into_iter().rev() {
        world.insert(entity, Velocity(3 * index + 1)).unwrap();
    }
    world
}

fn read_position(world: &World) -> u64 {
    world
        .query::<&Position>()
        .fold(0, |sum, (_, position)| sum.wrapping_add(position.0))
}

fn read_position_and_velocity(world: &World) -> u64 {
    world
        .query::<(&Position, &Velocity)>()
        .fold(0, |sum, (_, (position, velocity))| {
            sum.wrapping_add(position.0).wrapping_add(velocity.0)
        })
}

fn queries(c: &mut Criterion) {
    let mut world = populated_world();
    let expected_sum = ENTITY_COUNT * (ENTITY_COUNT - 1) / 2;
    assert_eq!(world.query::<&Position>().count() as u64, ENTITY_COUNT);
    assert_eq!(
        world.query::<(&Position, &Velocity)>().count() as u64,
        ENTITY_COUNT
    );
    assert_eq!(read_position(&world), expected_sum);
    assert_eq!(
        read_position_and_velocity(&world),
        4 * expected_sum + ENTITY_COUNT
    );

    let mut group = c.benchmark_group("world_query");
    group.throughput(Throughput::Elements(ENTITY_COUNT));
    group.bench_function("read_position", |b| {
        b.iter(|| black_box(read_position(black_box(&world))));
    });
    group.bench_function("read_position_and_velocity", |b| {
        b.iter(|| black_box(read_position_and_velocity(black_box(&world))));
    });
    group.bench_function("write_position", |b| {
        b.iter(|| {
            black_box(&mut world)
                .query_mut::<&mut Position>()
                .unwrap()
                .for_each(|_, position| position.0 = position.0.wrapping_add(1));
        });
    });

    let mut matches = 0;
    world
        .query_filtered_mut::<&mut Position, (With<Moving>, Without<Disabled>)>()
        .unwrap()
        .for_each(|_, _| matches += 1);
    assert_eq!(matches, ENTITY_COUNT / 4);
    group.throughput(Throughput::Elements(matches));
    group.bench_function("write_enabled_movers", |b| {
        b.iter(|| {
            black_box(&mut world)
                .query_filtered_mut::<&mut Position, (With<Moving>, Without<Disabled>)>()
                .unwrap()
                .for_each(|_, position| position.0 = position.0.wrapping_add(1));
        });
    });
    group.finish();
}

fn queued_lifecycle() -> CommandBuffer {
    let mut commands = CommandBuffer::new();
    let entities = (0..COMMAND_ENTITIES)
        .map(|index| {
            let entity = commands.spawn();
            commands.insert(&entity, Position(index));
            entity
        })
        .collect::<Vec<_>>();
    for entity in &entities {
        commands.despawn(entity);
    }
    commands
}

fn deferred_lifecycle(c: &mut Criterion) {
    let mut world = World::new();
    assert!(queued_lifecycle().flush(&mut world).failures().is_empty());
    assert_eq!(world.query::<&Position>().count(), 0);

    let mut group = c.benchmark_group("command_buffer");
    group.throughput(Throughput::Elements(3 * COMMAND_ENTITIES));
    group.bench_function("flush_spawn_insert_despawn", |b| {
        b.iter_batched_ref(
            queued_lifecycle,
            |commands| commands.flush(black_box(&mut world)),
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn cached_schedule(c: &mut Criterion) {
    let mut group = c.benchmark_group("cached_schedule");
    for systems in [1_u64, 8] {
        let mut world = World::new();
        let mut schedule = Schedule::new();
        for id in 0..systems {
            schedule.add_system(SystemAccess::default(), move |_, _| {
                black_box(id);
                Ok(())
            });
        }
        assert!(schedule.run(&mut world).unwrap().is_empty());
        group.throughput(Throughput::Elements(systems));
        group.bench_function(BenchmarkId::from_parameter(systems), |b| {
            b.iter(|| black_box(schedule.run(black_box(&mut world)).unwrap()));
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .without_plots()
        .sample_size(40)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = queries, deferred_lifecycle, cached_schedule
}

criterion_main!(benches);
