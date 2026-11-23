//! Deferred structural changes through the public ECS API.

use mythraan_ecs::prelude::{CommandBuffer, World};

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

    commands.flush(&mut world);
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

    commands.flush(&mut world);
    assert_eq!(world.get::<u32>(live), Some(&7));
}
