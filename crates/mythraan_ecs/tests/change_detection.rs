//! Change detection through the public ECS API.

use mythraan_ecs::prelude::{Added, Changed, Entity, World};

fn added(world: &mut World) -> Vec<Entity> {
    let mut matches = Vec::new();
    world
        .query_filtered_mut::<&u32, Added<u32>>()
        .unwrap()
        .for_each(|entity, _| {
            matches.push(entity);
        });
    matches
}

fn changed(world: &mut World) -> Vec<Entity> {
    let mut matches = Vec::new();
    world
        .query_filtered_mut::<&u32, Changed<u32>>()
        .unwrap()
        .for_each(|entity, _| {
            matches.push(entity);
        });
    matches
}

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

#[test]
fn adding_and_replacing_are_distinct_across_ticks() {
    let mut world = World::new();
    let existing = world.spawn().unwrap();
    let new = world.spawn().unwrap();
    assert!(added(&mut world).is_empty());
    world.insert(existing, 10_u32).unwrap();
    assert_eq!(added(&mut world), [existing]);

    let prior = world.change_tick();
    world.advance_change_tick().unwrap();
    world.insert(existing, 11_u32).unwrap();
    world.insert(new, 20_u32).unwrap();
    assert_eq!(added(&mut world), [new]);

    world.advance_change_tick().unwrap();
    let mut matches = Vec::new();
    world
        .query_filtered_mut_since::<&u32, Added<u32>>(prior)
        .unwrap()
        .for_each(|entity, _| {
            matches.push(entity);
        });
    assert_eq!(matches, [new]);
    assert!(added(&mut world).is_empty());
}

#[test]
fn added_ticks_follow_components_after_swap_removal_and_slot_reuse() {
    let mut world = World::new();
    let old = world.spawn().unwrap();
    let survivor = world.spawn().unwrap();
    world.insert(old, 10_u32).unwrap();
    world.advance_change_tick().unwrap();
    world.insert(survivor, 20_u32).unwrap();

    assert!(world.despawn(old));
    assert_eq!(added(&mut world), [survivor]);
    world.advance_change_tick().unwrap();
    let recycled = world.spawn().unwrap();
    assert_eq!(recycled.index(), old.index());
    assert_ne!(recycled, old);
    world.insert(recycled, 30_u32).unwrap();
    assert_eq!(added(&mut world), [recycled]);
}

#[test]
fn insert_replace_and_mutable_access_mark_changed_components() {
    let mut world = World::new();
    let borrowed = world.spawn().unwrap();
    let replaced = world.spawn().unwrap();
    world.insert(borrowed, 10_u32).unwrap();
    world.insert(replaced, 20_u32).unwrap();
    assert_eq!(changed(&mut world), [borrowed, replaced]);

    world.advance_change_tick().unwrap();
    assert_eq!(world.query::<&u32>().count(), 2);
    assert!(changed(&mut world).is_empty());
    assert_eq!(world.get_mut::<u32>(borrowed).as_deref(), Some(&10));
    world.insert(replaced, 21_u32).unwrap();
    assert_eq!(changed(&mut world), [borrowed, replaced]);
    assert!(added(&mut world).is_empty());
    world.advance_change_tick().unwrap();
    assert!(changed(&mut world).is_empty());
}
