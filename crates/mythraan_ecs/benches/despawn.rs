//! Measures component removal and entity despawn independently of fixture setup.

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{Bencher, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mythraan_ecs::prelude::{CommandBuffer, Entity, EntityAllocator, SparseSet, World};
use mythraan_math::rng::Rng;

const DENSE_ROWS: usize = 2_048;
const REMOVALS: usize = 256;
const DESPAWNS: usize = 2_048;
const SHUFFLE_SEED: u64 = 0x4d59_5448_4731_3463;

#[derive(Clone, Copy)]
enum Position {
    Last,
    Middle,
}

impl Position {
    fn cases() -> [(&'static str, Self); 2] {
        [("last", Self::Last), ("middle", Self::Middle)]
    }
}

fn measure_removals<S>(
    bencher: &mut Bencher<'_>,
    mut storage: S,
    mut dense: Vec<Entity>,
    position: Position,
    mut remove: impl FnMut(&mut S, Entity) -> Option<u64>,
    mut restore: impl FnMut(&mut S, Entity) -> Option<u64>,
) {
    let mut targets = Vec::with_capacity(REMOVALS);
    bencher.iter_custom(|iterations| {
        let mut measured = Duration::ZERO;
        for _ in 0..iterations {
            targets.clear();
            for _ in 0..REMOVALS {
                let index = match position {
                    Position::Last => dense.len() - 1,
                    Position::Middle => dense.len() / 2,
                };
                targets.push(dense.swap_remove(index));
            }

            let start = Instant::now();
            for &entity in &targets {
                black_box(remove(&mut storage, entity));
            }
            measured += start.elapsed();

            for &entity in &targets {
                assert_eq!(restore(&mut storage, entity), None);
            }
            dense.extend_from_slice(&targets);
        }
        measured
    });
}

fn sparse_fixture() -> (SparseSet<u64>, Vec<Entity>) {
    let mut allocator = EntityAllocator::new();
    let mut storage = SparseSet::new();
    let dense = (0..DENSE_ROWS)
        .map(|_| {
            let entity = allocator.spawn().unwrap();
            storage.insert(entity, u64::from(entity.index())).unwrap();
            entity
        })
        .collect();
    (storage, dense)
}

fn sparse_removal(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_remove_sparse");
    group.throughput(Throughput::Elements(REMOVALS as u64));
    for (name, position) in Position::cases() {
        group.bench_function(name, |bencher| {
            let (storage, dense) = sparse_fixture();
            measure_removals(
                bencher,
                storage,
                dense,
                position,
                SparseSet::remove,
                |storage, entity| storage.insert(entity, u64::from(entity.index())).unwrap(),
            );
        });
    }
    group.finish();
}

fn tracked_fixture() -> (World, Vec<Entity>) {
    let mut world = World::new();
    let dense = (0..DENSE_ROWS)
        .map(|_| {
            let entity = world.spawn().unwrap();
            world.insert(entity, u64::from(entity.index())).unwrap();
            entity
        })
        .collect();
    (world, dense)
}

fn tracked_removal(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_remove_world");
    group.throughput(Throughput::Elements(REMOVALS as u64));
    for (name, position) in Position::cases() {
        group.bench_function(name, |bencher| {
            let (world, dense) = tracked_fixture();
            measure_removals(
                bencher,
                world,
                dense,
                position,
                World::remove::<u64>,
                |world, entity| world.insert(entity, u64::from(entity.index())).unwrap(),
            );
        });
    }
    group.finish();
}

fn component_lookup(c: &mut Criterion) {
    let (storage, sparse_entities) = sparse_fixture();
    let (world, world_entities) = tracked_fixture();
    let first = (DENSE_ROWS - REMOVALS) / 2;
    let window = first..first + REMOVALS;
    let sparse_entities = &sparse_entities[window.clone()];
    let world_entities = &world_entities[window];
    let mut group = c.benchmark_group("component_lookup");
    group.throughput(Throughput::Elements(REMOVALS as u64));
    group.bench_function("sparse", |bencher| {
        bencher.iter(|| {
            for &entity in black_box(sparse_entities) {
                black_box(storage.get(entity).copied());
            }
        });
    });
    group.bench_function("world", |bencher| {
        bencher.iter(|| {
            for &entity in black_box(world_entities) {
                black_box(world.get::<u64>(entity).copied());
            }
        });
    });
    group.finish();
}

#[derive(Clone, Copy)]
enum Extra {
    Empty,
    Populated,
}

impl Extra {
    fn cases() -> [(&'static str, Self); 2] {
        [("empty", Self::Empty), ("populated", Self::Populated)]
    }
}

struct Component<const ID: usize> {
    _value: u64,
}

type Populate = fn(&mut World, &[Entity], Extra);

fn populate<const ID: usize>(world: &mut World, entities: &[Entity], contents: Extra) {
    let members = match contents {
        // Insert and remove at the high index so both modes cover the same sparse range.
        Extra::Empty => std::slice::from_ref(entities.last().unwrap()),
        Extra::Populated => entities,
    };
    for &entity in members {
        let value = Component::<ID> {
            _value: u64::from(entity.index()),
        };
        assert!(world.insert(entity, value).unwrap().is_none());
    }
    if let Extra::Empty = contents {
        assert!(world.remove::<Component<ID>>(members[0]).is_some());
    }
}

macro_rules! populators {
    ($($id:literal),+ $(,)?) => {
        [$(populate::<$id>),+]
    };
}

const STORAGES: [Populate; 50] = populators!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
);

#[derive(Clone, Copy)]
enum VisitOrder {
    Shuffled,
    Sequential,
}

struct DespawnFixture {
    world: World,
    targets: Vec<Entity>,
    shuffled: Vec<Entity>,
    order: Vec<usize>,
    target_storages: &'static [Populate],
}

impl DespawnFixture {
    fn new(storage_count: usize, components: usize, extra: Extra, visit_order: VisitOrder) -> Self {
        let mut world = World::new();
        let targets = (0..DESPAWNS).map(|_| world.spawn().unwrap()).collect();
        let others: Vec<_> = (0..DESPAWNS).map(|_| world.spawn().unwrap()).collect();
        let mut order: Vec<_> = (0..DESPAWNS).collect();
        if let VisitOrder::Shuffled = visit_order {
            let mut rng = Rng::from_seed(SHUFFLE_SEED);
            for index in (1..order.len()).rev() {
                let other = rng.range_u32(0..(index + 1) as u32).unwrap() as usize;
                order.swap(index, other);
            }
        }

        let mut fixture = Self {
            world,
            targets,
            shuffled: Vec::with_capacity(DESPAWNS),
            order,
            target_storages: &STORAGES[..components],
        };
        fixture.prepare_targets();
        for fill in &STORAGES[components..storage_count] {
            fill(&mut fixture.world, &others, extra);
        }
        fixture
    }

    fn prepare_targets(&mut self) {
        for fill in self.target_storages {
            fill(&mut self.world, &self.targets, Extra::Populated);
        }
        self.shuffled.clear();
        self.shuffled
            .extend(self.order.iter().map(|&row| self.targets[row]));
    }

    fn restore(&mut self) {
        // Reverse the free-list order to retain the original sparse indices.
        for &row in self.order.iter().rev() {
            let entity = self.world.spawn().unwrap();
            assert_eq!(entity.index(), self.targets[row].index());
            assert_ne!(entity.generation(), self.targets[row].generation());
            self.targets[row] = entity;
        }
        self.prepare_targets();
    }
}

#[derive(Clone, Copy)]
enum Despawn {
    Direct,
    Flush,
}

impl Despawn {
    fn cases() -> [(&'static str, Self); 2] {
        [("direct", Self::Direct), ("flush", Self::Flush)]
    }
}

fn measure_despawn(bencher: &mut Bencher<'_>, mut fixture: DespawnFixture, path: Despawn) {
    let mut queue = CommandBuffer::new();
    bencher.iter_custom(|iterations| {
        let mut measured = Duration::ZERO;
        for _ in 0..iterations {
            match path {
                Despawn::Direct => {
                    let start = Instant::now();
                    for &target in &fixture.shuffled {
                        black_box(fixture.world.despawn(target));
                    }
                    measured += start.elapsed();
                }
                Despawn::Flush => {
                    for &target in &fixture.shuffled {
                        queue.despawn(target);
                    }
                    let start = Instant::now();
                    let report = black_box(queue.flush(&mut fixture.world));
                    measured += start.elapsed();
                    assert!(report.failures().is_empty());
                }
            }
            fixture.restore();
        }
        measured
    });
}

fn despawn_storages(c: &mut Criterion) {
    despawn_axis(c, "despawn_storages", [4, 8, 16, 32, 50], |storages| {
        (storages, 4)
    });
}

fn despawn_components(c: &mut Criterion) {
    despawn_axis(c, "despawn_components", [1, 4, 16], |components| {
        (32, components)
    });
}

fn despawn_axis<const N: usize>(
    c: &mut Criterion,
    name: &str,
    mut counts: [usize; N],
    dimensions: impl Fn(usize) -> (usize, usize),
) {
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Elements(DESPAWNS as u64));
    let mut paths = Despawn::cases();
    let mut extras = Extra::cases();
    if std::env::var_os("MYTHRAAN_DESPAWN_REVERSE").is_some() {
        counts.reverse();
        paths.reverse();
        extras.reverse();
    }
    for (path_name, path) in paths {
        for (extra_name, extra) in extras {
            for count in counts {
                let (storages, components) = dimensions(count);
                let id = BenchmarkId::new(format!("{path_name}_{extra_name}"), count);
                group.bench_function(id, |bencher| {
                    let fixture =
                        DespawnFixture::new(storages, components, extra, VisitOrder::Shuffled);
                    measure_despawn(bencher, fixture, path);
                });
            }
        }
    }
    group.finish();
}

fn despawn_order(c: &mut Criterion) {
    let mut group = c.benchmark_group("despawn_order");
    group.throughput(Throughput::Elements(DESPAWNS as u64));
    let mut orders = [
        ("sequential", VisitOrder::Sequential),
        ("shuffled", VisitOrder::Shuffled),
    ];
    if std::env::var_os("MYTHRAAN_DESPAWN_REVERSE").is_some() {
        orders.reverse();
    }
    for (path_name, path) in Despawn::cases() {
        for (extra_name, extra) in Extra::cases() {
            for (order_name, order) in orders {
                let id = BenchmarkId::new(format!("{path_name}_{extra_name}"), order_name);
                group.bench_function(id, |bencher| {
                    measure_despawn(bencher, DespawnFixture::new(32, 4, extra, order), path);
                });
            }
        }
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .without_plots()
        .sample_size(30)
        .warm_up_time(Duration::from_millis(650))
        .measurement_time(Duration::from_millis(1_350));
    targets = sparse_removal, tracked_removal, component_lookup, despawn_storages, despawn_components, despawn_order
}

criterion_main!(benches);
