//! World behavior through the public ECS API.

use mythraan_ecs::prelude::World;

#[test]
fn spawns_distinct_live_entities() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();

    assert_ne!(first, second);
    assert!(world.is_alive(first));
    assert!(world.is_alive(second));
}

#[test]
fn insert_returns_previous_component_of_the_same_type() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    assert_eq!(world.insert(entity, String::from("first")).unwrap(), None);
    assert_eq!(world.insert(entity, 5_u32).unwrap(), None);
    assert_eq!(
        world
            .insert(entity, String::from("second"))
            .unwrap()
            .as_deref(),
        Some("first")
    );
}

#[test]
fn reads_multiple_component_types_without_mixing_entities() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    world.insert(first, String::from("first")).unwrap();
    world.insert(first, 100_u32).unwrap();
    world.insert(second, String::from("second")).unwrap();

    assert_eq!(
        world.get::<String>(first).map(String::as_str),
        Some("first")
    );
    assert_eq!(
        world.get::<String>(second).map(String::as_str),
        Some("second")
    );
    assert_eq!(world.get::<u32>(first), Some(&100));
    assert_eq!(world.get::<u32>(second), None);
    assert_eq!(world.get::<bool>(first), None);
}

#[test]
fn mutates_only_the_component_borrowed() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    world.insert(first, 10_u32).unwrap();
    world.insert(first, false).unwrap();
    world.insert(second, 20_u32).unwrap();

    *world.get_mut::<u32>(first).unwrap() += 1;
    assert_eq!(world.get::<u32>(first), Some(&11));
    assert_eq!(world.get::<u32>(second), Some(&20));
    assert_eq!(world.get::<bool>(first), Some(&false));
    assert!(world.get_mut::<bool>(second).is_none());
}

#[test]
fn removes_one_component_and_preserves_other_records() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    world.insert(first, String::from("first")).unwrap();
    world.insert(first, 11_u32).unwrap();
    world.insert(second, String::from("second")).unwrap();

    assert_eq!(world.remove::<String>(first).as_deref(), Some("first"));
    assert_eq!(world.remove::<String>(first), None);
    assert_eq!(world.get::<u32>(first), Some(&11));
    assert_eq!(
        world.get::<String>(second).map(String::as_str),
        Some("second")
    );
    assert!(world.is_alive(first));
}

#[test]
fn despawn_cleans_every_component_storage_before_slot_reuse() {
    let mut world = World::new();
    let dead = world.spawn().unwrap();
    let survivor = world.spawn().unwrap();
    world.insert(dead, String::from("old")).unwrap();
    world.insert(dead, 5_u32).unwrap();
    world.insert(survivor, 9_u32).unwrap();
    assert!(world.despawn(dead));

    let replacement = world.spawn().unwrap();
    assert_eq!(replacement.index(), dead.index());
    assert_ne!(replacement.generation(), dead.generation());
    assert!(world.get::<String>(replacement).is_none());
    assert!(world.get::<u32>(replacement).is_none());
    assert_eq!(
        world.insert(replacement, String::from("new")).unwrap(),
        None
    );
    assert_eq!(world.insert(replacement, 7_u32).unwrap(), None);
    assert_eq!(world.get::<u32>(survivor), Some(&9));
}
