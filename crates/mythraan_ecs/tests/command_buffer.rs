//! Deferred structural changes through the public ECS API.

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
