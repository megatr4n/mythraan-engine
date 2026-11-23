//! Allocator behavior through the public ECS API.

use mythraan_ecs::prelude::EntityAllocator;

#[test]
fn issues_distinct_live_entities() {
    let mut entities = EntityAllocator::new();
    let first = entities.spawn().unwrap();
    let second = entities.spawn().unwrap();

    assert_ne!(first.index(), second.index());
    assert!(entities.is_alive(first));
    assert!(entities.is_alive(second));
}

#[test]
fn reuses_a_slot_without_reviving_its_previous_entity() {
    let mut entities = EntityAllocator::new();
    let previous = entities.spawn().unwrap();
    assert!(entities.despawn(previous));
    assert!(!entities.is_alive(previous));

    let replacement = entities.spawn().unwrap();
    assert_eq!(replacement.index(), previous.index());
    assert_eq!(replacement.generation(), previous.generation() + 1);
    assert!(!entities.is_alive(previous));
    assert!(!entities.despawn(previous));
    assert!(entities.is_alive(replacement));
}
