//! Read-only query behavior through the public ECS API.

use mythraan_ecs::prelude::{Query, World};

#[derive(Debug, PartialEq, Eq)]
struct Health(u32);

#[derive(Debug, PartialEq, Eq)]
struct Team(u8);

#[test]
fn quadruple_query_returns_components_in_the_requested_order() {
    let mut world = World::new();
    let complete = world.spawn().unwrap();
    let partial = world.spawn().unwrap();
    for entity in [complete, partial] {
        world.insert(entity, Health(80)).unwrap();
        world.insert(entity, Position(12)).unwrap();
        world.insert(entity, Team(3)).unwrap();
    }
    world.insert(complete, Speed(5)).unwrap();

    let values: Vec<_> = world
        .query::<(&Team, &Speed, &Position, &Health)>()
        .collect();
    assert_eq!(
        values,
        [(complete, (&Team(3), &Speed(5), &Position(12), &Health(80)))]
    );
}

#[test]
fn one_element_tuple_yields_a_single_reference_in_a_tuple() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, Health(15)).unwrap();

    let values: Vec<_> = world.query::<(&Health,)>().collect();
    assert_eq!(values, [(entity, (&Health(15),))]);
}

#[test]
fn missing_and_empty_storages_produce_no_rows() {
    let mut world = World::new();
    assert!(world.query::<&Health>().next().is_none());

    let entity = world.spawn().unwrap();
    world.insert(entity, Health(12)).unwrap();
    assert!(world.query::<(&Health, &Position)>().next().is_none());
    world.insert(entity, Position(23)).unwrap();
    world.remove::<Position>(entity).unwrap();

    let mut empty = world.query::<(&Health, &Position)>();
    assert_eq!(empty.size_hint(), (0, Some(0)));
    assert!(empty.next().is_none());
}

#[test]
fn disjoint_storages_exhaust_the_query_without_matching() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    let third = world.spawn().unwrap();
    world.insert(first, Health(20)).unwrap();
    world.insert(second, Health(30)).unwrap();
    world.insert(third, Position(5)).unwrap();

    let mut query = world.query::<(&Health, &Position)>();
    assert_eq!(query.size_hint(), (0, Some(1)));
    assert!(query.next().is_none());
    assert_eq!(query.size_hint(), (0, Some(0)));
    assert!(query.next().is_none());
}

#[test]
fn single_query_reads_components_from_matching_entities() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let unrelated = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    world.insert(first, Health(40)).unwrap();
    world.insert(unrelated, 99_u32).unwrap();
    world.insert(second, Health(80)).unwrap();

    let query: Query<'_, &Health> = world.query();
    let mut values: Vec<_> = query.map(|(entity, health)| (entity, health.0)).collect();
    values.sort_unstable_by_key(|(entity, _)| entity.index());
    assert_eq!(values, [(first, 40), (second, 80)]);
}

#[derive(Debug, PartialEq, Eq)]
struct Position(i32);

#[test]
fn pair_query_joins_components_by_entity() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    let health_only = world.spawn().unwrap();
    let position_only = world.spawn().unwrap();
    world.insert(first, Health(40)).unwrap();
    world.insert(second, Health(80)).unwrap();
    world.insert(health_only, Health(10)).unwrap();
    world.insert(second, Position(22)).unwrap();
    world.insert(position_only, Position(50)).unwrap();
    world.insert(first, Position(11)).unwrap();

    let mut values: Vec<_> = world
        .query::<(&Health, &Position)>()
        .map(|(entity, (health, position))| (entity, health.0, position.0))
        .collect();
    values.sort_unstable_by_key(|(entity, _, _)| entity.index());
    assert_eq!(values, [(first, 40, 11), (second, 80, 22)]);
    assert_eq!(world.query::<(&Position, &Health)>().count(), 2);
}

#[derive(Debug, PartialEq, Eq)]
struct Speed(i32);

#[test]
fn triple_query_checks_every_required_component() {
    let mut world = World::new();
    let complete = world.spawn().unwrap();
    let partial = world.spawn().unwrap();
    world.insert(complete, Health(42)).unwrap();
    world.insert(complete, Position(21)).unwrap();
    world.insert(complete, Speed(7)).unwrap();
    world.insert(partial, Health(50)).unwrap();
    world.insert(partial, Speed(8)).unwrap();

    let values: Vec<_> = world.query::<(&Speed, &Health, &Position)>().collect();
    assert_eq!(
        values,
        [(complete, (&Speed(7), &Health(42), &Position(21)))]
    );
}
