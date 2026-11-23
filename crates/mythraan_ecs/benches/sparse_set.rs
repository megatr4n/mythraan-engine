//! Compares packed component iteration with scanning optional slots.

use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mythraan_ecs::prelude::{EntityAllocator, SparseSet};

const SLOTS: usize = 65_536;

fn fixture(live: usize) -> (SparseSet<u64>, Vec<Option<u64>>) {
    let mut allocator = EntityAllocator::new();
    let mut components = SparseSet::new();
    let mut optional = vec![None; SLOTS];

    for (index, slot) in optional.iter_mut().enumerate() {
        let entity = allocator.spawn().unwrap();
        // Permute the slot numbers to spread precisely `live` entries across them.
        let permuted = (index ^ (index >> 7)).wrapping_mul(40_503) & (SLOTS - 1);
        if permuted < live {
            let value = index as u64 + 1;
            components.insert(entity, value).unwrap();
            *slot = Some(value);
        }
    }

    assert_eq!(components.len(), live);
    assert_eq!(optional.iter().flatten().count(), live);
    assert_eq!(
        components.values().iter().sum::<u64>(),
        optional.iter().flatten().sum::<u64>()
    );
    (components, optional)
}

fn iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_iteration");
    for live in [8_192, 32_768, SLOTS] {
        let (components, optional) = fixture(live);
        group.throughput(Throughput::Elements(live as u64));
        group.bench_function(BenchmarkId::new("sparse_set", live), |b| {
            b.iter(|| {
                let sum = black_box(components.values())
                    .iter()
                    .copied()
                    .fold(0, u64::wrapping_add);
                black_box(sum)
            });
        });
        group.bench_function(BenchmarkId::new("vec_option", live), |b| {
            b.iter(|| {
                let sum = black_box(optional.as_slice())
                    .iter()
                    .flatten()
                    .copied()
                    .fold(0, u64::wrapping_add);
                black_box(sum)
            });
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
    targets = iteration
}

criterion_main!(benches);
