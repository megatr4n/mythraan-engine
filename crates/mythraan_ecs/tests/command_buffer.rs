//! Deferred structural changes through the public ECS API.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use mythraan_ecs::prelude::{CommandBuffer, World};
use mythraan_ecs::{CommandError, WorldInsertError};

#[test]
fn removals_wait_until_flush() {
    let mut world = World::new();
    let keep = world.spawn().unwrap();
    let discard = world.spawn().unwrap();
    world.insert(keep, 1_u32).unwrap();
    world.insert(discard, 2_u32).unwrap();
    world.insert(keep, true).unwrap();
    let mut commands = CommandBuffer::new();

    for (entity, value) in world.query::<&u32>() {
        if *value == 1 {
            commands.remove::<u32>(entity);
        } else {
            commands.despawn(entity);
        }
    }
    assert_eq!(world.query::<&u32>().count(), 2);
    assert!(world.is_alive(discard));

    assert!(commands.flush(&mut world).failures().is_empty());
    assert_eq!(world.query::<&u32>().count(), 0);
    assert!(!world.is_alive(discard));
    assert_eq!(world.get::<bool>(keep), Some(&true));
}

#[test]
fn deferred_removal_cannot_touch_a_recycled_entity() {
    let mut world = World::new();
    let stale = world.spawn().unwrap();
    let mut commands = CommandBuffer::new();
    commands.remove::<u32>(stale);
    commands.despawn(stale);

    assert!(world.despawn(stale));
    let live = world.spawn().unwrap();
    assert_eq!(live.index(), stale.index());
    world.insert(live, 7_u32).unwrap();

    assert!(commands.flush(&mut world).failures().is_empty());
    assert_eq!(world.get::<u32>(live), Some(&7));
}

#[test]
fn insert_and_remove_follow_submission_order() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    let mut commands = CommandBuffer::new();
    commands.insert(entity, 1_u32);
    commands.remove::<u32>(entity);
    commands.insert(entity, 3_u32);

    assert_eq!(world.get::<u32>(entity), None);
    assert!(commands.flush(&mut world).failures().is_empty());
    assert_eq!(world.get::<u32>(entity), Some(&3));
}

#[test]
fn a_failed_insert_does_not_skip_later_commands_or_replay() {
    let mut world = World::new();
    let live = world.spawn().unwrap();
    let dead = world.spawn().unwrap();
    let mut commands = CommandBuffer::new();
    commands.despawn(dead);
    commands.insert(dead, 1_u32);
    commands.insert(live, 2_u32);

    let report = commands.flush(&mut world);
    let [failure] = report.failures() else {
        panic!("expected one failed command, got {report:?}");
    };
    assert_eq!(failure.index, 1);
    assert!(matches!(
        failure.error,
        CommandError::Insert(WorldInsertError::EntityNotAlive { entity }) if entity == dead
    ));
    assert_eq!(world.remove::<u32>(live), Some(2));
    assert!(commands.flush(&mut world).failures().is_empty());
    assert_eq!(world.get::<u32>(live), None);
}

#[test]
fn queued_spawns_issue_identifiers_only_at_flush() {
    let mut world = World::new();
    let recycled = world.spawn().unwrap();
    assert!(world.despawn(recycled));

    let mut commands = CommandBuffer::new();
    let first = commands.spawn();
    let second = commands.spawn();
    let immediate = world.spawn().unwrap();
    assert_eq!(immediate.index(), recycled.index());

    let report = commands.flush(&mut world);
    assert!(report.failures().is_empty());
    let first = report.resolve(&first).unwrap();
    let second = report.resolve(&second).unwrap();
    assert!(immediate.index() < first.index());
    assert!(first.index() < second.index());
    assert!(world.is_alive(first));
    assert!(world.is_alive(second));
}

#[test]
fn spawn_reports_resolve_only_their_own_batch() {
    let mut world = World::new();
    let mut commands = CommandBuffer::new();
    let mut other = CommandBuffer::new();

    let first = commands.spawn();
    let foreign = other.spawn();
    let first_report = commands.flush(&mut world);
    let second = commands.spawn();
    let second_report = commands.flush(&mut world);

    assert!(first_report.resolve(&first).is_some());
    assert!(second_report.resolve(&second).is_some());
    assert_eq!(first_report.resolve(&second), None);
    assert_eq!(second_report.resolve(&first), None);
    assert_eq!(first_report.resolve(&foreign), None);
    assert_eq!(second_report.resolve(&foreign), None);
}

#[test]
fn placeholder_targets_apply_in_order_even_when_an_index_is_reused() {
    let mut world = World::new();
    let mut commands = CommandBuffer::new();

    let first = commands.spawn();
    commands.insert(&first, 13_u32);
    commands.despawn(&first);
    let second = commands.spawn();
    commands.insert(&second, 17_u32);
    commands.remove::<u32>(&second);
    commands.insert(&second, true);
    commands.remove::<bool>(&first);

    let report = commands.flush(&mut world);
    assert!(report.failures().is_empty());
    let first = report.resolve(&first).unwrap();
    let second = report.resolve(&second).unwrap();
    assert_eq!(first.index(), second.index());
    assert!(!world.is_alive(first));
    assert_eq!(world.get::<u32>(second), None);
    assert_eq!(world.get::<bool>(second), Some(&true));
}

#[test]
fn foreign_and_previous_batch_placeholders_cannot_target_a_local_spawn() {
    let mut world = World::new();
    let mut commands = CommandBuffer::new();
    let previous = commands.spawn();
    let previous_report = commands.flush(&mut world);
    let previous_live = previous_report.resolve(&previous).unwrap();

    let foreign = CommandBuffer::new().spawn();
    let local = commands.spawn();
    commands.insert(&foreign, 21_u32);
    commands.despawn(&previous);
    commands.insert(&local, true);

    let report = commands.flush(&mut world);
    let [foreign_failure, previous_failure] = report.failures() else {
        panic!("expected two failed commands, got {report:?}");
    };
    assert_eq!(foreign_failure.index, 1);
    assert_eq!(previous_failure.index, 2);
    assert!(matches!(
        foreign_failure.error,
        CommandError::ForeignPlaceholder
    ));
    assert!(matches!(
        previous_failure.error,
        CommandError::ForeignPlaceholder
    ));
    let local = report.resolve(&local).unwrap();
    assert_eq!(world.get::<bool>(local), Some(&true));
    assert_eq!(world.get::<u32>(local), None);
    assert!(world.is_alive(previous_live));
}

#[test]
fn mutable_queries_can_queue_all_structural_changes() {
    let mut world = World::new();
    let keep = world.spawn().unwrap();
    let discard = world.spawn().unwrap();
    world.insert(keep, 1_u32).unwrap();
    world.insert(discard, 2_u32).unwrap();
    world.insert(keep, true).unwrap();
    let mut commands = CommandBuffer::new();
    let mut replacements = Vec::new();

    world
        .query_mut::<&mut u32>()
        .unwrap()
        .for_each(|entity, value| {
            *value += 10;
            if entity == keep {
                commands.remove::<bool>(entity);
            } else {
                commands.despawn(entity);
                let replacement = commands.spawn();
                commands.insert(&replacement, *value);
                replacements.push(replacement);
            }
        });
    assert_eq!(world.get::<u32>(keep), Some(&11));
    assert_eq!(world.get::<u32>(discard), Some(&12));
    assert_eq!(world.get::<bool>(keep), Some(&true));
    assert_eq!(world.query::<&u32>().count(), 2);

    let report = commands.flush(&mut world);
    assert!(report.failures().is_empty());
    let [replacement] = replacements.as_slice() else {
        panic!("expected one queued replacement");
    };
    let replacement = report.resolve(replacement).unwrap();
    assert_eq!(world.get::<bool>(keep), None);
    assert!(!world.is_alive(discard));
    assert_eq!(world.get::<u32>(replacement), Some(&12));
    assert_eq!(world.query::<&u32>().count(), 2);
}

#[test]
fn dropping_a_buffer_releases_owned_queued_components() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    let component = Arc::new(());
    let observer = Arc::downgrade(&component);
    let mut commands = CommandBuffer::new();
    commands.insert(entity, component);

    assert!(observer.upgrade().is_some());
    drop(commands);
    assert!(observer.upgrade().is_none());
    assert!(world.get::<Arc<()>>(entity).is_none());
}

#[test]
fn unwinding_discards_pending_commands_and_leaves_the_buffer_reusable() {
    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("component destructor unwound");
        }
    }

    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, PanicOnDrop).unwrap();
    let mut commands = CommandBuffer::new();
    commands.remove::<PanicOnDrop>(entity);
    let interrupted = commands.spawn();
    commands.insert(entity, 1_u32);

    let result = catch_unwind(AssertUnwindSafe(|| commands.flush(&mut world)));
    assert!(result.is_err());
    assert!(world.get::<PanicOnDrop>(entity).is_none());

    let next = commands.spawn();
    let report = commands.flush(&mut world);
    assert!(report.failures().is_empty());
    assert_eq!(report.resolve(&interrupted), None);
    assert!(report.resolve(&next).is_some());
    assert_eq!(world.get::<u32>(entity), None);
}
