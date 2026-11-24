//! Measures component removal and entity despawn independently of fixture setup.

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{Bencher, Criterion, Throughput, criterion_group, criterion_main};
use mythraan_ecs::prelude::{Entity, EntityAllocator, SparseSet};

const DENSE_ROWS: usize = 2_048;
const REMOVALS: usize = 256;

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

criterion_group! {
    name = benches;
    config = Criterion::default()
        .without_plots()
        .sample_size(30)
        .warm_up_time(Duration::from_millis(650))
        .measurement_time(Duration::from_millis(1_350));
    targets = sparse_removal
}

criterion_main!(benches);
