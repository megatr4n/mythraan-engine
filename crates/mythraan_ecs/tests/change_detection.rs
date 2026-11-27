//! Change detection through the public ECS API.

use std::panic::{AssertUnwindSafe, catch_unwind};

use mythraan_ecs::QueryFilter;
use mythraan_ecs::prelude::{Added, Changed, CommandBuffer, Entity, With, Without, World};
use mythraan_math::rng::Rng;

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

#[test]
fn changed_filter_can_read_the_stamp_of_a_writable_component() {
    let mut world = World::new();
    let marked = world.spawn().unwrap();
    let untouched = world.spawn().unwrap();
    world.insert(marked, 10_u32).unwrap();
    world.insert(untouched, 20_u32).unwrap();
    world.advance_change_tick().unwrap();
    world.insert(marked, 11_u32).unwrap();

    world
        .query_filtered_mut::<&mut u32, Changed<u32>>()
        .unwrap()
        .for_each(|entity, value| {
            assert_eq!(entity, marked);
            *value += 1;
        });
    assert_eq!(world.get::<u32>(marked), Some(&12));
    assert_eq!(world.get::<u32>(untouched), Some(&20));
    assert_eq!(changed(&mut world), [marked]);
}

#[test]
fn dense_writes_keep_ticks_aligned_with_values_after_deferred_despawn() {
    let mut world = World::new();
    let [blocked, removed, survivor] = [(); 3].map(|()| world.spawn().unwrap());
    for (entity, value) in [(blocked, 10_u32), (removed, 20), (survivor, 30)] {
        world.insert(entity, value).unwrap();
    }
    world.insert(blocked, true).unwrap();
    let mut commands = CommandBuffer::new();
    commands.despawn(removed);
    assert!(commands.flush(&mut world).failures().is_empty());
    world.advance_change_tick().unwrap();

    world
        .query_filtered_mut::<&mut u32, Without<bool>>()
        .unwrap()
        .for_each(|entity, value| {
            assert_eq!(entity, survivor);
            *value += 1;
        });
    assert_eq!(world.get::<u32>(blocked), Some(&10));
    assert_eq!(world.get::<u32>(survivor), Some(&31));
    assert_eq!(changed(&mut world), [survivor]);
}

#[test]
fn missing_tuple_component_does_not_mark_unvisited_rows() {
    let mut world = World::new();
    let matched = world.spawn().unwrap();
    let incomplete = world.spawn().unwrap();
    world.insert(matched, 10_u32).unwrap();
    world.insert(incomplete, 20_u32).unwrap();
    world.insert(matched, true).unwrap();
    for _ in 0..2 {
        let other = world.spawn().unwrap();
        world.insert(other, true).unwrap();
    }
    world.advance_change_tick().unwrap();
    world
        .query_mut::<(&mut u32, &bool)>()
        .unwrap()
        .for_each(|entity, (value, flag)| {
            assert_eq!(entity, matched);
            assert!(*flag);
            *value += 1;
        });
    assert_eq!(changed(&mut world), [matched]);
}

#[test]
fn change_windows_compose_with_presence_filters_and_skip_unchanged_rows() {
    let mut world = World::new();
    let matching = world.spawn().unwrap();
    let unchanged = world.spawn().unwrap();
    let untagged = world.spawn().unwrap();
    let excluded = world.spawn().unwrap();
    for entity in [matching, unchanged, untagged, excluded] {
        world.insert(entity, 1_u32).unwrap();
    }
    for entity in [matching, unchanged, excluded] {
        world.insert(entity, true).unwrap();
    }
    world.insert(excluded, ()).unwrap();
    let checkpoint = world.change_tick();

    world.advance_change_tick().unwrap();
    for entity in [matching, untagged, excluded] {
        world.insert(entity, 2_u32).unwrap();
    }
    world.advance_change_tick().unwrap();
    assert!(changed(&mut world).is_empty());

    let mut matches = Vec::new();
    world
        .query_filtered_mut_since::<&bool, (Changed<u32>, With<bool>, Without<()>)>(checkpoint)
        .unwrap()
        .for_each(|entity, _| matches.push(entity));
    assert_eq!(matches, [matching]);
}

#[test]
fn deferred_writes_are_tracked_at_flush_time() {
    let mut world = World::new();
    let existing = world.spawn().unwrap();
    world.insert(existing, 1_u32).unwrap();
    world.advance_change_tick().unwrap();

    let mut commands = CommandBuffer::new();
    let new = commands.spawn();
    commands.insert(&new, 2_u32);
    commands.insert(existing, 3_u32);
    assert!(added(&mut world).is_empty());
    assert!(changed(&mut world).is_empty());

    world.advance_change_tick().unwrap();
    let report = commands.flush(&mut world);
    assert!(report.failures().is_empty());
    let new = report.resolve(&new).unwrap();
    assert_eq!(added(&mut world), [new]);
    let matches = changed(&mut world);
    assert_eq!(matches.len(), 2);
    assert!(matches.contains(&new));
    assert!(matches.contains(&existing));
    assert_eq!(world.get::<u32>(existing), Some(&3));
    world.advance_change_tick().unwrap();
    assert!(added(&mut world).is_empty());
    assert!(changed(&mut world).is_empty());
}

struct NeverInserted;

fn dense_tick_fixture(values: &[(u32, bool)], removed: usize) -> World {
    let mut world = World::new();
    let entities: Vec<_> = values
        .iter()
        .map(|&(value, blocked)| {
            let entity = world.spawn().unwrap();
            world.insert(entity, value).unwrap();
            if blocked {
                world.insert(entity, true).unwrap();
            }
            entity
        })
        .collect();
    let old = entities[removed];
    world.advance_change_tick().unwrap();
    world
        .query_filtered_mut::<&mut u32, Without<bool>>()
        .unwrap()
        .for_each(|_, value| *value = value.wrapping_add(1));

    world.advance_change_tick().unwrap();
    let mut commands = CommandBuffer::new();
    commands.despawn(old);
    let spawn = commands.spawn();
    commands.insert(&spawn, 17_u32);
    let report = commands.flush(&mut world);
    assert!(report.failures().is_empty());
    let replacement = report.resolve(&spawn).unwrap();
    assert_eq!(old.index(), replacement.index());
    assert_ne!(old, replacement);
    assert!(world.get::<u32>(old).is_none());
    world
}

fn observed_since<F: QueryFilter>(world: &mut World, after: u64) -> Vec<(Entity, u32)> {
    let mut rows = Vec::new();
    world
        .query_filtered_mut_since::<&u32, F>(after)
        .unwrap()
        .for_each(|entity, value| rows.push((entity, *value)));
    rows
}

fn assert_tick_windows_match(actual: &mut World, reference: &mut World) {
    for after in 0..=actual.change_tick() {
        assert_eq!(
            observed_since::<Changed<u32>>(actual, after),
            observed_since::<Changed<u32>>(reference, after),
            "Changed after tick {after}"
        );
        assert_eq!(
            observed_since::<Added<u32>>(actual, after),
            observed_since::<Added<u32>>(reference, after),
            "Added after tick {after}"
        );
    }
}

#[test]
fn dense_mutable_traversal_matches_per_row_stamping_across_worlds() {
    let seeds = if cfg!(miri) { 8 } else { 256 };
    for seed in 0..seeds {
        let mut rng = Rng::from_seed(seed);
        let len = 3 + rng.next_u32() as usize % if cfg!(miri) { 16 } else { 256 };
        let values: Vec<_> = (0..len)
            .map(|_| (rng.next_u32(), rng.next_u32().is_multiple_of(2)))
            .collect();
        let removed = rng.next_u32() as usize % (len - 1);
        let mut actual = dense_tick_fixture(&values, removed);
        let mut reference = dense_tick_fixture(&values, removed);

        // Only the shared query implements Iterator. A partial read must leave
        // the old change window intact before the exclusive traversal starts.
        let before_read = changed(&mut actual);
        assert_eq!(
            actual.query::<&u32>().next(),
            reference.query::<&u32>().next()
        );
        assert_eq!(changed(&mut actual), before_read);
        assert_tick_windows_match(&mut actual, &mut reference);

        actual.advance_change_tick().unwrap();
        reference.advance_change_tick().unwrap();
        actual.query_mut::<&mut u32>().unwrap().for_each(|_, _| {});
        // This filter admits every row and retains the old per-row stamp path.
        reference
            .query_filtered_mut::<&mut u32, Without<NeverInserted>>()
            .unwrap()
            .for_each(|_, _| {});
        assert_eq!(changed(&mut actual).len(), len);
        assert!(added(&mut actual).is_empty());
        assert_tick_windows_match(&mut actual, &mut reference);

        actual.advance_change_tick().unwrap();
        reference.advance_change_tick().unwrap();
        let limit = 1 + rng.next_u32() as usize % (len - 1);
        let mut visits = Vec::new();
        let mut reference_visits = Vec::new();
        for (world, visited, use_reference) in [
            (&mut actual, &mut visits, false),
            (&mut reference, &mut reference_visits, true),
        ] {
            let result = catch_unwind(AssertUnwindSafe(|| {
                let query = if use_reference {
                    world.query_filtered_mut::<&mut u32, Without<NeverInserted>>()
                } else {
                    world.query_mut::<&mut u32>()
                };
                query.unwrap().for_each(|entity, value| {
                    visited.push(entity);
                    assert_ne!(visited.len(), limit, "stop after yielding the row");
                    *value = value.wrapping_add(1);
                });
            }));
            assert!(result.is_err());
            assert_eq!(visited.len(), limit);
            assert_eq!(changed(world), *visited);
        }
        assert_eq!(visits, reference_visits);
        assert_tick_windows_match(&mut actual, &mut reference);
    }
}
