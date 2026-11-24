//! Measures ECS queries, deferred mutations, and cached schedule execution.

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{
    BatchSize, BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use mythraan_ecs::prelude::{
    CommandBuffer, DeferredEntity, Entity, Schedule, SystemAccess, With, Without, World,
};

const ENTITY_COUNT: u64 = 100_000;
const COMMAND_ENTITIES: u64 = 1_024;

struct Position(u64);
struct Velocity(u64);
struct Moving;
struct Disabled;

fn populated_world(reverse_velocity: bool) -> World {
    let mut world = World::new();
    let mut entities = (0..ENTITY_COUNT)
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

    if reverse_velocity {
        entities.reverse();
    }
    for (entity, index) in entities {
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
    let mut world = populated_world(true);
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
    let aligned = populated_world(false);
    assert_eq!(
        read_position_and_velocity(&aligned),
        4 * expected_sum + ENTITY_COUNT
    );
    group.throughput(Throughput::Elements(ENTITY_COUNT));
    group.bench_function("read_position_and_velocity_aligned", |b| {
        b.iter(|| black_box(read_position_and_velocity(black_box(&aligned))));
    });
    group.finish();
}

fn query_setup(c: &mut Criterion) {
    let mut world = populated_world(true);
    let mut group = c.benchmark_group("world_query_setup");
    group.bench_function("read_position_and_velocity", |b| {
        b.iter(|| black_box(black_box(&world).query::<(&Position, &Velocity)>()));
    });
    group.bench_function("write_enabled_movers", |b| {
        b.iter(|| {
            let _ = black_box(
                black_box(&mut world)
                    .query_filtered_mut::<&mut Position, (With<Moving>, Without<Disabled>)>()
                    .unwrap(),
            );
        });
    });
    group.finish();
}

struct ChurnFrame<const PERIOD: usize> {
    world: World,
    live: Vec<Entity>,
    commands: CommandBuffer,
    replacements: Vec<DeferredEntity>,
    phase: usize,
}

impl<const PERIOD: usize> ChurnFrame<PERIOD> {
    fn new() -> Self {
        let world = populated_world(true);
        let live = world
            .query::<&Position>()
            .map(|(entity, _)| entity)
            .collect();
        let mut frame = Self {
            world,
            live,
            commands: CommandBuffer::new(),
            replacements: Vec::with_capacity(ENTITY_COUNT as usize / PERIOD),
            phase: 0,
        };
        let retired = frame.live[0];
        for _ in 0..PERIOD {
            frame.advance();
        }
        assert!(!frame.world.is_alive(retired));
        assert_eq!(
            frame.world.query::<(&Position, &Velocity)>().count() as u64,
            ENTITY_COUNT
        );
        assert_eq!(
            read_position_and_velocity(&frame.world),
            2 * ENTITY_COUNT * (ENTITY_COUNT - 1) + ENTITY_COUNT
        );
        frame
    }

    fn advance(&mut self) {
        self.world.advance_change_tick().unwrap();
        let phase = self.phase;
        // Stagger the selection in pairs of groups to preserve the tag mix in every frame.
        let selected = (0..self.live.len() / PERIOD)
            .map(move |group| group * PERIOD + (phase + group / 2) % PERIOD);
        for index in selected.clone() {
            self.commands.despawn(self.live[index]);
        }
        for index in selected.clone() {
            let spawned = self.commands.spawn();
            self.commands.insert(&spawned, Position(index as u64));
            if index % 2 == 0 {
                self.commands.insert(&spawned, Moving);
            }
            if index % 4 == 0 {
                self.commands.insert(&spawned, Disabled);
            }
            self.replacements.push(spawned);
        }
        // Separate insertion streams keep exercising different dense orders after slot reuse.
        for (index, entity) in selected.clone().zip(&self.replacements).rev() {
            self.commands.insert(entity, Velocity(3 * index as u64 + 1));
        }

        let report = self.commands.flush(&mut self.world);
        assert!(report.failures().is_empty());
        for (index, entity) in selected.zip(self.replacements.drain(..)) {
            self.live[index] = report.resolve(&entity).unwrap();
        }
        self.phase = (self.phase + 1) % PERIOD;
    }
}

fn frames_with_churn(c: &mut Criterion) {
    let mut group = c.benchmark_group("world_frame");
    group.throughput(Throughput::Elements(ENTITY_COUNT));
    bench_churn::<10>(&mut group, "read_after_churn", "churn_and_read");
    bench_churn::<100>(&mut group, "read_after_churn_1pct", "churn_and_read_1pct");
    group.finish();
}

fn bench_churn<const PERIOD: usize>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    read_case: &str,
    frame_case: &str,
) {
    let mut frame = ChurnFrame::<PERIOD>::new();
    group.bench_function(read_case, |b| {
        b.iter_custom(|iterations| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                frame.advance();
                let start = Instant::now();
                black_box(read_position_and_velocity(black_box(&frame.world)));
                elapsed += start.elapsed();
            }
            elapsed
        });
    });
    let mut frame = ChurnFrame::<PERIOD>::new();
    group.bench_function(frame_case, |b| {
        b.iter(|| {
            frame.advance();
            black_box(read_position_and_velocity(black_box(&frame.world)));
        });
    });
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
    targets = queries, query_setup, frames_with_churn, deferred_lifecycle, cached_schedule
}

criterion_main!(benches);
