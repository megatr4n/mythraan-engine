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
