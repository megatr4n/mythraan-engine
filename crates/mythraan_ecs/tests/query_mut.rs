//! Mutable query behavior through the public ECS API.

use mythraan_ecs::prelude::{With, Without, World};

#[derive(Debug, PartialEq, Eq)]
struct Position(i32);

struct Velocity(i32);

struct Player;
struct Frozen;

#[test]
fn optional_queries_respect_filters_on_live_entities() {
    let mut world = World::new();
    let marked = world.spawn().unwrap();
    let empty = world.spawn().unwrap();
    let dead = world.spawn().unwrap();
    world.insert(marked, Player).unwrap();
    assert!(world.despawn(dead));

    let mut matches = Vec::new();
    world
        .query_filtered_mut::<Option<&Velocity>, With<Player>>()
        .unwrap()
        .for_each(|id, velocity| {
            assert!(velocity.is_none());
            matches.push(id);
        });
    assert_eq!(matches, [marked]);
    matches.clear();
    world
        .query_filtered_mut::<Option<&Velocity>, Without<Player>>()
        .unwrap()
        .for_each(|id, velocity| {
            assert!(velocity.is_none());
            matches.push(id);
        });
    assert_eq!(matches, [empty]);
}

#[test]
fn a_missing_required_store_or_impossible_filter_yields_no_rows() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, Position(10)).unwrap();

    world
        .query_mut::<(&mut Position, &Velocity)>()
        .unwrap()
        .for_each(|_, _| {
            panic!("the required velocity store is missing");
        });
    world
        .query_filtered_mut::<&mut Position, With<Frozen>>()
        .unwrap()
        .for_each(|_, _| {
            panic!("the required frozen store is missing");
        });
    world
        .query_filtered_mut::<&mut Position, (With<Position>, Without<Position>)>()
        .unwrap()
        .for_each(|_, _| panic!("a row cannot both have and lack a position"));
    assert_eq!(world.get::<Position>(entity), Some(&Position(10)));
}

#[test]
fn optional_reads_work_with_two_mutable_columns() {
    struct Updates(u32);

    let mut world = World::new();
    let with_velocity = world.spawn().unwrap();
    let without_velocity = world.spawn().unwrap();
    for entity in [with_velocity, without_velocity] {
        world.insert(entity, Position(10)).unwrap();
        world.insert(entity, Updates(0)).unwrap();
        world.insert(entity, Player).unwrap();
    }
    world.insert(with_velocity, Velocity(5)).unwrap();

    world
        .query_mut::<(&Player, &mut Position, Option<&Velocity>, &mut Updates)>()
        .unwrap()
        .for_each(|_, (_, position, velocity, updates)| {
            position.0 += velocity.map_or(1, |velocity| velocity.0);
            updates.0 += 1;
        });
    assert_eq!(world.get::<Position>(with_velocity), Some(&Position(15)));
    assert_eq!(world.get::<Position>(without_velocity), Some(&Position(11)));
    for entity in [with_velocity, without_velocity] {
        assert_eq!(
            world.get::<Updates>(entity).map(|updates| updates.0),
            Some(1)
        );
    }
}

#[test]
fn duplicate_shared_fields_are_allowed() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, Position(7)).unwrap();
    let mut count = 0;
    world
        .query_mut::<(&Position, Option<&Position>)>()
        .unwrap()
        .for_each(|_, (left, right)| {
            assert!(std::ptr::eq(left, right.unwrap()));
            count += 1;
        });
    assert_eq!(count, 1);
}

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
fn a_presence_driver_keeps_the_other_membership_checks() {
    let mut world = World::new();
    let [player_only, ready_a, ready_b, blocked, extra_a, extra_b] =
        [(); 6].map(|()| world.spawn().unwrap());
    for entity in [ready_a, ready_b, blocked, extra_a, extra_b] {
        world.insert(entity, Position(0)).unwrap();
    }
    for entity in [ready_b, player_only, blocked, ready_a] {
        world.insert(entity, Player).unwrap();
    }
    world.insert(blocked, Frozen).unwrap();

    let mut visited = Vec::new();
    world
        .query_filtered_mut::<&mut Position, (With<Player>, Without<Frozen>)>()
        .unwrap()
        .for_each(|entity, position| {
            position.0 += 1;
            visited.push(entity);
        });
    assert_eq!(visited, [ready_b, ready_a]);
    assert_eq!(world.get::<Position>(ready_a), Some(&Position(1)));
    assert_eq!(world.get::<Position>(ready_b), Some(&Position(1)));
    assert_eq!(world.get::<Position>(blocked), Some(&Position(0)));
    assert_eq!(world.get::<Position>(player_only), None);
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
