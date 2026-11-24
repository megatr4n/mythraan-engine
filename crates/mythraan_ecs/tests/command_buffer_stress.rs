//! Reused deferred queues checked over many generations of the same slots.

use std::sync::atomic::{AtomicUsize, Ordering};

use mythraan_ecs::prelude::{CommandBuffer, World};
use mythraan_ecs::{CommandError, WorldInsertError};

const BATCHES: usize = if cfg!(miri) { 12 } else { 96 };
const WIDTH: usize = if cfg!(miri) { 8 } else { 24 };

#[derive(Debug, PartialEq, Eq)]
struct Stamp(usize, usize);

#[test]
fn repeated_flushes_isolate_placeholders_and_recycle_generations() {
    let mut world = World::new();
    let mut queue = CommandBuffer::new();
    let mut live = Vec::new();
    let mut previous = None;

    for batch in 0..BATCHES {
        for &entity in &live {
            queue.despawn(entity);
        }
        let mut first_pair = live.len();
        let foreign = CommandBuffer::new().spawn();
        queue.insert(&foreign, Stamp(batch, WIDTH));
        first_pair += 1;
        if let Some(placeholder) = previous.take() {
            queue.insert(&placeholder, Stamp(batch, WIDTH));
            first_pair += 1;
        }

        let mut pairs = Vec::new();
        for slot in 0..WIDTH {
            let stale = queue.spawn();
            queue.insert(&stale, Stamp(batch, slot));
            queue.despawn(&stale);
            let replacement = queue.spawn();
            queue.insert(&stale, Stamp(batch, WIDTH));
            queue.insert(&replacement, Stamp(batch, slot));
            pairs.push((stale, replacement));
        }

        let report = queue.flush(&mut world);
        assert_eq!(report.resolve(&foreign), None);
        assert!(live.iter().all(|&entity| !world.is_alive(entity)));
        let mut failures = report.failures().iter();
        for index in live.len()..first_pair {
            let failure = failures.next().unwrap();
            assert_eq!(failure.index, index);
            assert!(matches!(failure.error, CommandError::ForeignPlaceholder));
        }
        live.clear();
        for (slot, (old, replacement)) in pairs.iter().enumerate() {
            let old = report.resolve(old).unwrap();
            let entity = report.resolve(replacement).unwrap();
            let failure = failures.next().unwrap();
            assert_eq!(failure.index, first_pair + slot * 6 + 4);
            assert!(matches!(
                failure.error,
                CommandError::Insert(WorldInsertError::EntityNotAlive { entity }) if entity == old
            ));
            assert!(!world.is_alive(old));
            assert_eq!(entity.index(), old.index());
            assert_eq!(entity.generation(), old.generation() + 1);
            assert_eq!(world.get::<Stamp>(entity), Some(&Stamp(batch, slot)));
            live.push(entity);
        }
        assert!(failures.next().is_none());
        assert_eq!(world.query::<&Stamp>().count(), WIDTH);
        assert!(queue.flush(&mut world).failures().is_empty());
        previous = pairs.pop().map(|(_, latest)| latest);
    }
}

#[test]
fn reused_batches_preserve_alignment_and_drop_zero_sized_payloads_once() {
    #[repr(align(256))]
    struct Aligned(usize);

    #[repr(align(64))]
    struct Marker;

    static MARKER_DROPS: AtomicUsize = AtomicUsize::new(0);

    impl Drop for Marker {
        fn drop(&mut self) {
            MARKER_DROPS.fetch_add(1, Ordering::Relaxed);
        }
    }

    let mut world = World::new();
    let mut queue = CommandBuffer::new();
    let targets: Vec<_> = (0..WIDTH).map(|_| world.spawn().unwrap()).collect();
    let mut marker_drops = 0;

    for batch in 0..BATCHES {
        let count = if batch % 2 == 0 { WIDTH } else { WIDTH / 2 };
        for (slot, &entity) in targets[..count].iter().enumerate() {
            queue.insert(entity, Aligned(batch * WIDTH + slot));
            queue.insert(entity, Marker);
            queue.insert(entity, slot as u8);
        }
        assert!(queue.flush(&mut world).failures().is_empty());
        assert_eq!(MARKER_DROPS.load(Ordering::Relaxed), marker_drops);

        for (slot, &entity) in targets[..count].iter().enumerate() {
            let aligned = world.get::<Aligned>(entity).unwrap();
            assert!(std::ptr::from_ref(aligned).is_aligned());
            assert_eq!(aligned.0, batch * WIDTH + slot);
            assert_eq!(world.get::<u8>(entity), Some(&(slot as u8)));
            assert!(std::ptr::from_ref(world.get::<Marker>(entity).unwrap()).is_aligned());

            queue.remove::<Aligned>(entity);
            queue.remove::<Marker>(entity);
            queue.remove::<u8>(entity);
        }
        assert!(queue.flush(&mut world).failures().is_empty());
        marker_drops += count;
        assert_eq!(MARKER_DROPS.load(Ordering::Relaxed), marker_drops);
        assert_eq!(world.query::<&Aligned>().count(), 0);
    }

    queue.insert(targets[0], Marker);
    drop(queue);
    assert_eq!(MARKER_DROPS.load(Ordering::Relaxed), marker_drops + 1);
}
