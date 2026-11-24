//! Fixed-trace churn frames and separate structural mutation timings for G1.5.

use std::hint::black_box;
use std::time::Instant;

use mythraan_ecs::prelude::{CommandBuffer, DeferredEntity, Entity, World};
use mythraan_math::rng::Rng;

const ROWS: usize = 100_000;
const SEED: u64 = 0x4d59_5448_4731_3463;

struct Component<const ID: usize>(u64);

struct Operations {
    insert: fn(&mut World, Entity, u64),
    remove: fn(&mut World, Entity),
    queue: fn(&mut CommandBuffer, &DeferredEntity, u64),
    refresh: fn(&mut CommandBuffer, Entity, u64),
}

impl Operations {
    const fn for_type<const ID: usize>() -> Self {
        Self {
            insert: |world, entity, value| {
                world.insert(entity, Component::<ID>(value)).unwrap();
            },
            remove: |world, entity| {
                assert!(world.remove::<Component<ID>>(entity).is_some());
            },
            queue: |commands, entity, value| commands.insert(entity, Component::<ID>(value)),
            refresh: |commands, entity, value| {
                commands.remove::<Component<ID>>(entity);
                commands.insert(entity, Component::<ID>(value));
            },
        }
    }
}

macro_rules! operations {
    ($($id:literal),*) => { [$(Operations::for_type::<$id>()),*] };
}

const TYPES: [Operations; 50] = operations!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49
);

fn shuffle(rows: usize) -> Vec<usize> {
    let mut order: Vec<_> = (0..rows).collect();
    let mut rng = Rng::from_seed(SEED);
    for i in (1..rows).rev() {
        let other = rng.range_u32(0..(i + 1) as u32).unwrap() as usize;
        order.swap(i, other);
    }
    order
}

struct Row {
    entity: Entity,
    types: [usize; 5],
    len: usize,
}

struct Scene {
    world: World,
    rows: Vec<Row>,
    order: Vec<usize>,
    commands: CommandBuffer,
    pending: Vec<DeferredEntity>,
    cursor: usize,
}

impl Scene {
    fn new(types: usize) -> Self {
        assert!((32..=50).contains(&types));
        let mut world = World::new();
        let mut rng = Rng::from_seed(SEED);
        let mut histogram = vec![0; types];
        let rows = (0..ROWS)
            .map(|row| {
                let entity = world.spawn().unwrap();
                let len = 3 + rng.range_u32(0..3).unwrap() as usize;
                let mut ids = [0, if row % 4 == 0 { 2 } else { 1 }, 0, 0, 0];
                for slot in 2..len {
                    loop {
                        let id = 3 + rng.range_u32(0..(types - 3) as u32).unwrap() as usize;
                        if !ids[..slot].contains(&id) {
                            ids[slot] = id;
                            break;
                        }
                    }
                }
                for &id in &ids[..len] {
                    (TYPES[id].insert)(&mut world, entity, row as u64);
                    histogram[id] += 1;
                }
                Row {
                    entity,
                    types: ids,
                    len,
                }
            })
            .collect();
        assert!(histogram.iter().all(|&count| count > 0));
        Self {
            world,
            rows,
            order: shuffle(ROWS),
            commands: CommandBuffer::new(),
            pending: Vec::new(),
            cursor: 0,
        }
    }

    // A shuffled ring visits every logical row once per cycle. Survivors
    // in the opposite half are disjoint from this frame's replacements.
    fn frame(&mut self, n: usize) -> u64 {
        self.world.advance_change_tick().unwrap();
        let selected = &self.order[self.cursor..self.cursor + n];
        for &i in selected {
            self.commands.despawn(self.rows[i].entity);
        }
        for &i in selected {
            let row = &self.rows[i];
            let next = self.commands.spawn();
            for &id in &row.types[..row.len] {
                (TYPES[id].queue)(&mut self.commands, &next, i as u64);
            }
            self.pending.push(next);
        }
        let survivor = (self.cursor + ROWS / 2) % ROWS;
        for &i in &self.order[survivor..survivor + n] {
            let row = &self.rows[i];
            (TYPES[row.types[row.len - 1]].refresh)(&mut self.commands, row.entity, i as u64);
        }
        let report = self.commands.flush(&mut self.world);
        assert!(report.failures().is_empty());
        for (&i, next) in selected.iter().zip(self.pending.drain(..)) {
            self.rows[i].entity = report.resolve(&next).unwrap();
        }
        self.cursor = (self.cursor + n) % ROWS;
        self.world
            .query::<(&Component<0>, &Component<1>)>()
            .fold(0, |sum, (_, (p, v))| sum + p.0 + v.0)
    }

    fn costs(&mut self, n: usize) -> [f64; 3] {
        let selected = &self.order[self.cursor..self.cursor + n];
        let start = Instant::now();
        for &i in selected {
            let row = &self.rows[i];
            (TYPES[row.types[row.len - 1]].remove)(&mut self.world, row.entity);
        }
        let remove = start.elapsed();
        let start = Instant::now();
        for &i in selected {
            let row = &self.rows[i];
            (TYPES[row.types[row.len - 1]].insert)(&mut self.world, row.entity, i as u64);
        }
        let insert = start.elapsed();
        let start = Instant::now();
        for &i in selected {
            assert!(self.world.despawn(self.rows[i].entity));
        }
        let despawn = start.elapsed();
        for &i in selected.iter().rev() {
            let row = &mut self.rows[i];
            row.entity = self.world.spawn().unwrap();
            for &id in &row.types[..row.len] {
                (TYPES[id].insert)(&mut self.world, row.entity, i as u64);
            }
        }
        self.cursor = (self.cursor + n) % ROWS;
        [despawn, insert, remove].map(|duration| duration.as_secs_f64() * 1e9 / n as f64)
    }
}

#[inline(never)]
fn profile_despawn(world: &mut World, targets: &[Entity]) {
    for &entity in targets {
        assert!(black_box(world.despawn(entity)));
    }
}

fn profile(types: usize) {
    assert!([4, 50].contains(&types));
    let mut world = World::new();
    let targets: Vec<_> = (0..2048).map(|_| world.spawn().unwrap()).collect();
    let others: Vec<_> = (0..2048).map(|_| world.spawn().unwrap()).collect();
    for (id, ty) in TYPES[..types].iter().enumerate() {
        for &e in if id < 4 { &targets } else { &others } {
            (ty.insert)(&mut world, e, u64::from(e.index()));
        }
    }
    let shuffled: Vec<_> = shuffle(targets.len()).iter().map(|&i| targets[i]).collect();
    profile_despawn(&mut world, &shuffled);
}

fn sample(types: usize, percent: usize) {
    assert!([1, 10].contains(&percent));
    let n = ROWS * percent / 100;
    let mut scene = Scene::new(types);
    let expected: u64 = (0..ROWS as u64).filter(|i| i % 4 != 0).map(|i| 2 * i).sum();
    let warmup = ROWS / n;
    for _ in 0..warmup {
        assert_eq!(scene.frame(n), expected);
    }
    // Measure whole frames without interleaved microbenchmarks or fixture construction.
    for batch in 0..12 {
        let start = Instant::now();
        for _ in 0..5 {
            assert_eq!(black_box(scene.frame(n)), expected);
        }
        println!(
            "frame,{types},{percent},{batch},{}",
            start.elapsed().as_secs_f64() * 1e6 / 5.0
        );
    }
    for batch in 0..12 {
        let costs = scene.costs(n);
        for (label, value) in ["despawn", "insert", "remove"].into_iter().zip(costs) {
            println!("{label},{types},{percent},{batch},{value}");
        }
    }
}

fn main() {
    let args: Vec<_> = std::env::args()
        .skip(1)
        .filter(|arg| arg != "--bench")
        .collect();
    match args.as_slice() {
        [mode, types] if mode == "profile" => profile(types.parse().unwrap()),
        [mode, types, percent] if mode == "sample" => {
            sample(types.parse().unwrap(), percent.parse().unwrap())
        }
        [] => {
            for types in [32, 50] {
                for percent in [1, 10] {
                    sample(types, percent);
                }
            }
        }
        _ => panic!("usage: membership [sample <types> <percent> | profile <types>]"),
    }
}
