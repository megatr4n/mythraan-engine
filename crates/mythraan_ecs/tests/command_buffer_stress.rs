//! Reused deferred queues checked over many generations of the same slots.

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
