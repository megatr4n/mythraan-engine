//! Change detection through the public ECS API.

use mythraan_ecs::prelude::World;

#[test]
fn change_ticks_start_at_one_and_advance_independently() {
    let mut first = World::new();
    let other = World::default();

    assert_eq!(first.change_tick(), 1);
    assert_eq!(first.advance_change_tick().unwrap(), 2);
    assert_eq!(first.advance_change_tick().unwrap(), 3);
    assert_eq!(first.change_tick(), 3);
    assert_eq!(other.change_tick(), 1);
}
