//! Read-only query behavior through the public ECS API.

use mythraan_ecs::prelude::{CommandBuffer, Entity, Query, World};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

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
fn query_reads_only_current_entities_after_recycling() {
    let mut world = World::new();
    let dead = world.spawn().unwrap();
    let partial = world.spawn().unwrap();
    let survivor = world.spawn().unwrap();
    for (entity, value) in [(dead, 3), (partial, 5), (survivor, 7)] {
        world.insert(entity, Health(value)).unwrap();
        world.insert(entity, Position(value as i32 * 10)).unwrap();
    }
    world.remove::<Position>(partial).unwrap();
    assert!(world.despawn(dead));
    let replacement = world.spawn().unwrap();
    assert_eq!(replacement.index(), dead.index());
    assert_ne!(replacement.generation(), dead.generation());
    world.insert(replacement, Position(90)).unwrap();

    let rows: Vec<_> = world.query::<(&Position, &Health)>().collect();
    assert_eq!(rows, [(survivor, (&Position(70), &Health(7)))]);
    world.insert(replacement, Health(9)).unwrap();
    let mut rows: Vec<_> = world.query::<(&Position, &Health)>().collect();
    rows.sort_unstable_by_key(|(entity, _)| entity.index());
    assert_eq!(
        rows,
        [
            (replacement, (&Position(90), &Health(9))),
            (survivor, (&Position(70), &Health(7))),
        ]
    );
}

#[test]
fn reads_can_overlap_and_repeat_a_component_type() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, Health(35)).unwrap();

    let mut first = world.query::<&Health>();
    let mut second = world.query::<(&Health, &Health)>();
    let (first_id, health) = first.next().unwrap();
    let (second_id, (left, right)) = second.next().unwrap();
    assert_eq!((first_id, second_id), (entity, entity));
    assert!(std::ptr::eq(health, left));
    assert!(std::ptr::eq(health, right));
    assert!(first.next().is_none());
    assert!(second.next().is_none());
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

struct Sample<const ID: usize>(u32);

fn populate_column<const ID: usize>(world: &mut World, rows: &[(Entity, u8, u16)]) {
    for offset in 0..rows.len() {
        let index = if ID.is_multiple_of(2) {
            offset
        } else {
            rows.len() - 1 - offset
        };
        let (entity, mask, sample) = rows[index];
        if mask & (1 << ID) != 0 {
            let value = u32::from(sample) * 4 + ID as u32;
            world.insert(entity, Sample::<ID>(value)).unwrap();
        }
    }
}

fn assert_sparse_join(world: &World, handles: &[Entity]) {
    let rare_rows = handles
        .iter()
        .filter(|&&entity| world.get::<Sample<0>>(entity).is_some())
        .count();

    macro_rules! assert_join {
        ($($id:literal => $value:ident),+ $(,)?) => {{
            let key = |entity: Entity| (entity.index(), entity.generation());
            let mut expected: Vec<_> = handles.iter().filter_map(|&entity| {
                Some((entity, ($(world.get::<Sample<$id>>(entity)?.0,)+)))
            }).collect();
            expected.sort_unstable_by_key(|&(entity, _)| key(entity));

            let mut query = world.query::<($(&Sample<$id>,)+)>();
            assert_eq!(query.size_hint(), (0, Some(rare_rows)));
            let mut actual: Vec<_> = (&mut query)
                .map(|(entity, ($($value,)+))| (entity, ($($value.0,)+)))
                .collect();
            actual.sort_unstable_by_key(|&(entity, _)| key(entity));
            assert_eq!(actual, expected);
            assert_eq!(query.size_hint(), (0, Some(0)));

            let mut remaining = world.query::<($(&Sample<$id>,)+)>();
            let project = |(entity, ($($value,)+)): (Entity, ($(&Sample<$id>,)+))| {
                (entity, ($($value.0,)+))
            };
            let head: Vec<_> = remaining.next().map(project).into_iter().collect();
            let mut folded = remaining.fold(head, |mut rows, entry| {
                rows.push(project(entry));
                rows
            });
            folded.sort_unstable_by_key(|&(entity, _)| key(entity));
            assert_eq!(folded, expected);
        }};
    }

    assert_join!(0 => a);
    assert_join!(1 => b, 0 => a);
    assert_join!(1 => b, 2 => c, 0 => a);
    assert_join!(0 => a, 1 => b, 2 => c, 3 => d);
    assert_join!(1 => b, 0 => a, 2 => c, 3 => d);
    assert_join!(1 => b, 2 => c, 0 => a, 3 => d);
    assert_join!(1 => b, 2 => c, 3 => d, 0 => a);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: if cfg!(miri) { 6 } else { 256 },
        rng_seed: RngSeed::Fixed(0x4d59_5448_4452_4956),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn tuple_queries_match_direct_reads_across_deferred_despawns(
        random in prop::collection::vec(
            (any::<u8>(), any::<u16>(), any::<bool>()),
            0..=if cfg!(miri) { 12 } else { 96 },
        ),
    ) {
        let mut world = World::new();
        // Keep column 0 strictly shortest and include a row missing its secondary components.
        let anchors = [(15, 11), (15, 22), (1, 33), (14, 44), (14, 55)];
        let mut rows: Vec<_> = anchors.into_iter().map(|(mask, value)| {
            (world.spawn().unwrap(), mask, value)
        }).collect();
        let mut dead = vec![rows[1].0];
        for (mask, value, despawn) in random {
            let entity = world.spawn().unwrap();
            let mask = if mask & 1 == 0 { mask & 14 } else { 15 };
            rows.push((entity, mask, value));
            if despawn {
                dead.push(entity);
            }
        }

        populate_column::<0>(&mut world, &rows);
        populate_column::<1>(&mut world, &rows);
        populate_column::<2>(&mut world, &rows);
        populate_column::<3>(&mut world, &rows);
        let mut handles: Vec<_> = rows.iter().map(|&(entity, _, _)| entity).collect();

        let mut commands = CommandBuffer::new();
        for &entity in &dead {
            commands.despawn(entity);
        }
        let pending = commands.spawn();
        commands.insert(&pending, Sample::<1>(111));
        commands.insert(&pending, Sample::<2>(222));
        commands.insert(&pending, Sample::<3>(333));
        assert_sparse_join(&world, &handles);

        let report = commands.flush(&mut world);
        assert!(report.failures().is_empty());
        assert!(dead.iter().all(|&entity| !world.is_alive(entity)));
        let reused = report.resolve(&pending).unwrap();
        let previous = dead.last().unwrap();
        assert_eq!(reused.index(), previous.index());
        assert_ne!(reused.generation(), previous.generation());
        handles.push(reused);
        assert_sparse_join(&world, &handles);
    }
}
