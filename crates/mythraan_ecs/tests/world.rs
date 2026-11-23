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
