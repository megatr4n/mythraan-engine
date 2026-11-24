use crate::{CommandBuffer, CommandTarget, Entity};

use super::{MEMBERSHIP_THRESHOLD, World};
use mythraan_math::{
    rng::Rng,
    transform::{GlobalTransform, Transform},
};

struct Value<const ID: usize>(u64);
struct Op {
    put: fn(&mut World, Entity, u64) -> Option<u64>,
    remove: fn(&mut World, Entity) -> Option<u64>,
    queue: fn(&mut CommandBuffer, CommandTarget<'_>, u64),
    erase: fn(&mut CommandBuffer, Entity),
    rows: fn(&World) -> Vec<(Entity, u64)>,
}
impl Op {
    const fn new<const ID: usize>() -> Self {
        Self {
            put: |w, e, v| w.insert(e, Value::<ID>(v)).unwrap().map(|old| old.0),
            remove: |w, e| w.remove::<Value<ID>>(e).map(|v| v.0),
            queue: |q, e, v| q.insert(e, Value::<ID>(v)),
            erase: |q, e| q.remove::<Value<ID>>(e),
            rows: |w| w.query::<&Value<ID>>().map(|(e, v)| (e, v.0)).collect(),
        }
    }
}
macro_rules! ops { ($($id:literal),*)=>{[$(Op::new::<$id>()),*]}; }
const OPS: [Op; 70] = ops!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69
);
fn assert_membership(world: &World, entity: Entity) {
    assert_eq!(
        world.membership.is_some(),
        world.components.len() >= MEMBERSHIP_THRESHOLD
    );
    if let Some(membership) = &world.membership {
        membership.assert_matches(&world.components, entity);
    }
}

fn put(rows: &mut Vec<(Entity, u64)>, e: Entity, v: u64) -> Option<u64> {
    if let Some((_, old)) = rows.iter_mut().find(|(key, _)| *key == e) {
        return Some(std::mem::replace(old, v));
    }
    rows.push((e, v));
    None
}
fn remove(rows: &mut Vec<(Entity, u64)>, e: Entity) -> Option<u64> {
    let i = rows.iter().position(|(key, _)| *key == e)?;
    Some(rows.swap_remove(i).1)
}
#[test]
fn mixed_lifecycles_match_dense_reference() {
    for seed in 0..if cfg!(miri) { 3 } else { 256 } {
        let mut rng = Rng::from_seed(seed);
        let mut w = World::new();
        let mut q = CommandBuffer::new();
        let mut live: Vec<_> = (0..12).map(|_| w.spawn().unwrap()).collect();
        let mut reference = vec![Vec::new(); OPS.len()];
        let mut types: Vec<_> = (0..OPS.len()).collect();
        for i in (1..types.len()).rev() {
            types.swap(i, rng.range_u32(0..i as u32 + 1).unwrap() as usize);
        }
        for &id in &types[..MEMBERSHIP_THRESHOLD - 1] {
            let e = live[rng.next_u32() as usize % live.len()];
            (OPS[id].put)(&mut w, e, seed);
            put(&mut reference[id], e, seed);
        }
        let steps_per_mode = if cfg!(miri) { 4 } else { 48 };
        for step in 0..2 * steps_per_mode {
            let limit = if step < steps_per_mode {
                MEMBERSHIP_THRESHOLD - 1
            } else {
                OPS.len()
            };
            if step == steps_per_mode {
                for &id in &types[MEMBERSHIP_THRESHOLD - 1..] {
                    let entity = live[rng.next_u32() as usize % live.len()];
                    (OPS[id].put)(&mut w, entity, seed);
                    put(&mut reference[id], entity, seed);
                    assert_membership(&w, entity);
                }
            }
            let row = rng.next_u32() as usize % live.len();
            let e = live[row];
            let id = types[rng.next_u32() as usize % limit];
            let v = rng.next_u64();
            match rng.next_u32() % 6 {
                0 => assert_eq!((OPS[id].put)(&mut w, e, v), put(&mut reference[id], e, v)),
                1 => assert_eq!((OPS[id].remove)(&mut w, e), remove(&mut reference[id], e)),
                2 => {
                    (OPS[id].queue)(&mut q, e.into(), v);
                    assert!(q.flush(&mut w).failures().is_empty());
                    put(&mut reference[id], e, v);
                }
                3 => {
                    (OPS[id].erase)(&mut q, e);
                    assert!(q.flush(&mut w).failures().is_empty());
                    remove(&mut reference[id], e);
                }
                other => {
                    for rows in &mut reference {
                        remove(rows, e);
                    }
                    let next = if other == 4 {
                        assert!(w.despawn(e));
                        assert_membership(&w, e);
                        let next = w.spawn().unwrap();
                        (OPS[id].put)(&mut w, next, v);
                        next
                    } else {
                        q.despawn(e);
                        let next = q.spawn();
                        (OPS[id].queue)(&mut q, (&next).into(), v);
                        (OPS[id].queue)(&mut q, e.into(), v);
                        let report = q.flush(&mut w);
                        assert_eq!(report.failures().len(), 1);
                        report.resolve(&next).unwrap()
                    };
                    assert_eq!(e.index(), next.index());
                    assert!(!w.is_alive(e));
                    put(&mut reference[id], next, v);
                    live[row] = next;
                }
            }
            for (id, rows) in reference.iter().enumerate() {
                assert_eq!(
                    (OPS[id].rows)(&w),
                    *rows,
                    "seed={seed} step={step} type={id}"
                );
            }
            for &e in &live {
                assert_membership(&w, e);
            }
        }
    }
}
#[test]
fn derived_remove_and_subtree_cleanup_update_membership() {
    let mut w = World::new();
    let parent = w.spawn().unwrap();
    let child = w.spawn().unwrap();
    w.set_parent(child, Some(parent)).unwrap();
    for op in &OPS[..MEMBERSHIP_THRESHOLD] {
        (op.put)(&mut w, child, 1);
    }
    w.insert(child, Transform::IDENTITY).unwrap();
    w.propagate_transforms().unwrap();
    assert!(w.get::<GlobalTransform>(child).is_some());
    assert_membership(&w, child);
    w.remove::<Transform>(child);
    assert!(w.get::<GlobalTransform>(child).is_none());
    assert_membership(&w, child);
    w.insert(child, Transform::IDENTITY).unwrap();
    w.propagate_transforms().unwrap();
    assert!(w.despawn(parent));
    assert_membership(&w, parent);
    assert_membership(&w, child);
}

#[test]
fn a_single_row_survives_a_second_word_and_index_reuse() {
    let mut w = World::new();
    let e = w.spawn().unwrap();
    for (i, op) in OPS.iter().enumerate() {
        assert_eq!((op.put)(&mut w, e, i as u64), None);
    }
    for i in [0, 62, 63, 64, 69] {
        assert_eq!((OPS[i].remove)(&mut w, e), Some(i as u64));
        assert_membership(&w, e);
    }
    assert!(w.despawn(e));
    assert_membership(&w, e);
    let next = w.spawn().unwrap();
    assert_eq!(e.index(), next.index());
    assert_membership(&w, next);
}

#[test]
fn panic_during_despawn_preserves_order_and_remaining_membership() {
    use std::{
        any::TypeId,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{Arc, Mutex},
    };
    struct Record<const N: usize>(Arc<Mutex<Vec<TypeId>>>, TypeId);
    impl<const N: usize> Drop for Record<N> {
        fn drop(&mut self) {
            let id = TypeId::of::<Self>();
            self.0.lock().unwrap().push(id);
            if self.1 == id {
                panic!("intentional destructor failure");
            }
        }
    }
    let mut expected = [
        TypeId::of::<Record<0>>(),
        TypeId::of::<Record<1>>(),
        TypeId::of::<Record<2>>(),
    ];
    expected.sort();
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut w = World::new();
    let e = w.spawn().unwrap();
    let other = w.spawn().unwrap();
    for op in &OPS[..MEMBERSHIP_THRESHOLD] {
        (op.put)(&mut w, other, 1);
    }
    w.insert(e, Record::<2>(log.clone(), expected[1])).unwrap();
    w.insert(e, Record::<0>(log.clone(), expected[1])).unwrap();
    w.insert(e, Record::<1>(log.clone(), expected[1])).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| w.despawn(e))).is_err());
    assert!(w.is_alive(e));
    assert_membership(&w, e);
    assert!(w.despawn(e));
    assert_membership(&w, e);
    assert_eq!(*log.lock().unwrap(), expected);
}

#[test]
fn flush_activates_on_new_type_between_recycling_commands() {
    let mut world = World::new();
    let doomed = world.spawn().unwrap();
    let survivor = world.spawn().unwrap();
    let hole = world.spawn().unwrap();
    for op in &OPS[..31] {
        (op.put)(&mut world, doomed, 1);
        (op.put)(&mut world, survivor, 2);
    }
    assert!(world.membership.is_none());
    assert!(world.despawn(hole));
    let mut commands = CommandBuffer::new();
    commands.despawn(doomed);
    let recycled = commands.spawn();
    (OPS[31].queue)(&mut commands, (&recycled).into(), 3);
    (OPS[0].queue)(&mut commands, doomed.into(), 4);
    (OPS[30].erase)(&mut commands, survivor);
    commands.despawn(survivor);
    let next = commands.spawn();
    (OPS[0].queue)(&mut commands, (&next).into(), 5);
    let report = commands.flush(&mut world);
    assert_eq!(report.failures().len(), 1);
    assert_eq!(report.failures()[0].index, 3);
    let recycled = report.resolve(&recycled).unwrap();
    let next = report.resolve(&next).unwrap();
    assert_eq!(recycled.index(), doomed.index());
    assert_eq!(next.index(), survivor.index());
    assert!(world.membership.is_some());
    assert!(!world.is_alive(doomed));
    assert!(!world.is_alive(survivor));
    assert!(world.get::<Value<0>>(recycled).is_none());
    assert_eq!(world.get::<Value<31>>(recycled).unwrap().0, 3);
    assert_eq!(world.get::<Value<0>>(next).unwrap().0, 5);
    for entity in [recycled, next, hole] {
        assert_membership(&world, entity);
    }
}

#[test]
fn late_registration_extends_a_plane_without_touching_existing_rows() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    let high = (0..80).map(|_| world.spawn().unwrap()).last().unwrap();
    for op in &OPS[..64] {
        (op.put)(&mut world, first, 1);
        (op.put)(&mut world, second, 2);
    }
    assert!(world.despawn(first));
    let recycled = world.spawn().unwrap();
    assert_eq!(first.index(), recycled.index());
    let mut commands = CommandBuffer::new();
    (OPS[64].queue)(&mut commands, high.into(), 3);
    (OPS[69].queue)(&mut commands, second.into(), 4);
    assert!(commands.flush(&mut world).failures().is_empty());
    for entity in [second, recycled, high] {
        assert_membership(&world, entity);
    }
    assert_eq!(world.get::<Value<0>>(second).unwrap().0, 2);
    assert!(world.get::<Value<64>>(recycled).is_none());
    (OPS[64].put)(&mut world, recycled, 5);
    assert_eq!((OPS[64].remove)(&mut world, high), Some(3));
    assert_membership(&world, recycled);
    assert_eq!(world.get::<Value<64>>(recycled).unwrap().0, 5);
}

#[test]
fn failed_reservation_preserves_scan_and_active_membership() {
    use crate::{SparseSetError, WorldInsertError};

    use super::membership::FAIL_NEXT_WORD_RESERVE;
    let mut world = World::new();
    let owner = world.spawn().unwrap();
    let target = world.spawn().unwrap();
    for op in &OPS[..31] {
        (op.put)(&mut world, owner, 7);
    }

    FAIL_NEXT_WORD_RESERVE.set(true);
    assert!(matches!(
        world.insert(target, Value::<31>(8)),
        Err(WorldInsertError::Storage(SparseSetError::Reserve(_)))
    ));
    assert_eq!(world.components.len(), 31);
    assert!(world.membership.is_none());
    assert!(world.get::<Value<31>>(target).is_none());
    assert_eq!(world.get::<Value<0>>(owner).unwrap().0, 7);
    assert!(world.despawn(owner));
    assert!(world.query::<&Value<0>>().next().is_none());
    world.insert(target, Value::<31>(8)).unwrap();
    assert_membership(&world, target);

    for op in &OPS[32..64] {
        (op.put)(&mut world, target, 9);
    }
    FAIL_NEXT_WORD_RESERVE.set(true);
    assert!(matches!(
        world.insert(target, Value::<64>(10)),
        Err(WorldInsertError::Storage(SparseSetError::Reserve(_)))
    ));
    assert_eq!(world.components.len(), 64);
    assert!(world.get::<Value<64>>(target).is_none());
    assert_membership(&world, target);
    world.insert(target, Value::<64>(10)).unwrap();
    assert_eq!(world.get::<Value<64>>(target).unwrap().0, 10);
    assert_membership(&world, target);
    assert!(world.despawn(target));
    assert_membership(&world, target);
}
