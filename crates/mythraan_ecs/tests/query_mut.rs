//! Mutable query behavior through the public ECS API.

use mythraan_ecs::prelude::{With, Without, World};

#[derive(Debug, PartialEq, Eq)]
struct Position(i32);

struct Velocity(i32);

struct Player;
struct Frozen;

#[test]
fn combined_filters_update_only_matching_entities() {
    let mut world = World::new();
    let moving = world.spawn().unwrap();
    let frozen = world.spawn().unwrap();
    let other = world.spawn().unwrap();
    for entity in [moving, frozen, other] {
        world.insert(entity, Position(10)).unwrap();
    }
    world.insert(moving, Player).unwrap();
    world.insert(frozen, Player).unwrap();
    world.insert(frozen, Frozen).unwrap();

    world
        .query_filtered_mut::<&mut Position, (With<Player>, Without<Frozen>)>()
        .unwrap()
        .for_each(|_, position| {
            position.0 += 1;
        });
    assert_eq!(world.get::<Position>(moving), Some(&Position(11)));
    assert_eq!(world.get::<Position>(frozen), Some(&Position(10)));
    assert_eq!(world.get::<Position>(other), Some(&Position(10)));
}

#[test]
fn checking_presence_does_not_conflict_with_mutation() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, Position(7)).unwrap();
    world
        .query_filtered_mut::<&mut Position, With<Position>>()
        .unwrap()
        .for_each(|_, position| {
            position.0 += 1;
        });
    assert_eq!(world.get::<Position>(entity), Some(&Position(8)));
}

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
