//! Measures arena insertion, lookup, reuse, and iteration.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mythraan_core::prelude::{Arena, Handle};

const COUNT: usize = 8192;

fn populated() -> (Arena<usize>, Vec<Handle<usize>>) {
    let mut arena = Arena::new();
    let handles = (0..COUNT).map(|value| arena.insert(value)).collect();
    (arena, handles)
}

fn insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_insert");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("fill_8192", |b| {
        b.iter(|| {
            let mut arena = Arena::new();
            for value in 0..COUNT {
                drop(black_box(arena.insert(black_box(value))));
            }
            black_box(arena)
        });
    });
    group.finish();
}

fn get(c: &mut Criterion) {
    let (arena, handles) = populated();
    let mut group = c.benchmark_group("arena_lookup");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("permuted_8192", |b| {
        b.iter(|| {
            for index in 0..COUNT {
                let permuted = index * 4051 % COUNT;
                black_box(arena.get(black_box(&handles[permuted])));
            }
        });
    });
    group.finish();
}

fn reuse(c: &mut Criterion) {
    let mut arena = Arena::new();
    let mut handle = arena.insert(0usize);
    let mut group = c.benchmark_group("arena_remove_insert");
    group.bench_function("reused_slot", |b| {
        b.iter(|| {
            let value = arena.remove(black_box(&handle)).unwrap();
            handle = arena.insert(black_box(value.wrapping_add(1)));
        });
    });
    group.finish();
}

fn iteration(c: &mut Criterion) {
    let (arena, _handles) = populated();
    let mut group = c.benchmark_group("arena_iteration");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("owned_8192", |b| {
        b.iter(|| {
            for (handle, value) in black_box(&arena).iter() {
                black_box((black_box(&handle).index(), *value));
            }
        });
    });
    group.bench_function("borrowed_8192", |b| {
        b.iter(|| {
            for (handle, value) in black_box(&arena).iter_borrowed() {
                black_box((black_box(&handle).index(), *value));
            }
        });
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .without_plots()
        .sample_size(40)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = insert, get, reuse, iteration
}
criterion_main!(benches);
