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
