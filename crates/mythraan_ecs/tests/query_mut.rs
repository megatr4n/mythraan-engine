//! Mutable query behavior through the public ECS API.

use mythraan_ecs::prelude::World;

#[derive(Debug, PartialEq, Eq)]
struct Position(i32);

struct Velocity(i32);

#[test]
fn updates_only_entities_with_every_required_component() {
    let mut world = World::new();
    let moving = world.spawn().unwrap();
    let stationary = world.spawn().unwrap();
    world.insert(moving, Position(5)).unwrap();
    world.insert(moving, Velocity(3)).unwrap();
    world.insert(stationary, Position(7)).unwrap();

    world
        .query_mut::<(&mut Position, &Velocity)>()
        .unwrap()
        .for_each(|_, (position, velocity)| {
            position.0 += velocity.0;
        });

    assert_eq!(world.get::<Position>(moving), Some(&Position(8)));
    assert_eq!(world.get::<Position>(stationary), Some(&Position(7)));
}

#[test]
fn optional_components_keep_entities_with_missing_storage() {
    let mut world = World::new();
    let empty = world.spawn().unwrap();
    let retired = world.spawn().unwrap();
    assert!(world.despawn(retired));
    let mut rows = Vec::new();
    world
        .query_mut::<Option<&Velocity>>()
        .unwrap()
        .for_each(|entity, velocity| {
            rows.push((entity, velocity.map(|velocity| velocity.0)));
        });
    assert_eq!(rows, [(empty, None)]);
}

#[test]
fn rejects_conflicts_even_before_components_exist() {
    let mut world = World::new();
    assert!(world.query_mut::<(&mut Position, &Position)>().is_err());
    assert!(world.query_mut::<(&mut Position, &mut Position)>().is_err());
    assert!(
        world
            .query_mut::<(Option<&Position>, &mut Position)>()
            .is_err()
    );
}
