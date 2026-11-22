//! Measures BVH construction and ray queries on fixed scenes.

use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mythraan_math::glam::Vec3;
use mythraan_math::prelude::{Aabb, Bvh, Ray};

const SIDES: [u32; 2] = [32, 128];
const RAY_COUNT: u32 = 256;
const SPACING: f32 = 2.0;

fn scene(side: u32) -> Vec<(Aabb, u32)> {
    let half = Vec3::splat(0.4);
    (0..side * side)
        .map(|id| {
            let center = Vec3::new(
                (id % side) as f32 * SPACING,
                (id / side) as f32 * SPACING,
                (id * 37 % 31) as f32,
            );
            (
                Aabb {
                    min: center - half,
                    max: center + half,
                },
                id,
            )
        })
        .collect()
}

fn rays(side: u32) -> Vec<Ray> {
    (0..RAY_COUNT)
        .map(|index| {
            let cell = index * 73 % (side * side);
            let column = cell % side;
            let gap = if index % 2 == 0 {
                0.0
            } else if column == side - 1 {
                -0.75
            } else {
                0.75
            };
            Ray {
                origin: Vec3::new(
                    column as f32 * SPACING + gap,
                    (cell / side) as f32 * SPACING,
                    -10.0,
                ),
                direction: Vec3::Z,
            }
        })
        .collect()
}

fn build(c: &mut Criterion) {
    let mut group = c.benchmark_group("bvh_build");
    for side in SIDES {
        let items = scene(side);
        group.throughput(Throughput::Elements(items.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(items.len()),
            &items,
            |b, items| {
                b.iter(|| Bvh::build(black_box(items.iter().copied())));
            },
        );
    }
    group.finish();
}

fn raycast(c: &mut Criterion) {
    let mut group = c.benchmark_group("bvh_raycast_batch");
    group.throughput(Throughput::Elements(u64::from(RAY_COUNT)));
    for side in SIDES {
        let tree = Bvh::build(scene(side));
        let queries = rays(side);
        let hits = queries
            .iter()
            .filter(|ray| tree.raycast(ray).is_some())
            .count();
        assert_eq!(hits, RAY_COUNT as usize / 2);

        group.bench_function(BenchmarkId::from_parameter(side * side), |b| {
            b.iter(|| {
                for ray in black_box(&queries) {
                    black_box(tree.raycast(ray));
                }
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
    targets = build, raycast
}
criterion_main!(benches);
