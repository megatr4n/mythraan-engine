//! Read-only query behavior through the public ECS API.

use std::num::NonZeroUsize;

use mythraan_ecs::prelude::{CommandBuffer, Entity, Query, With, Without, World};
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

#[test]
fn single_query_fold_pairs_the_remaining_entities_and_values() {
    let mut world = World::new();
    let entities = [(); 3].map(|()| world.spawn().unwrap());
    let [first, second, third] = entities;
    for (entity, health) in [(third, 30), (first, 10), (second, 20)] {
        world.insert(entity, Health(health)).unwrap();
    }

    let mut query = world.query::<&Health>();
    assert_eq!(query.next(), Some((third, &Health(30))));
    let remaining = query.fold(Vec::new(), |mut rows, (entity, health)| {
        rows.push((entity, health.0));
        rows
    });
    assert_eq!(remaining, [(first, 10), (second, 20)]);
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

// The old representation resolves a one-based sparse position and compares
// the full dense entity to reject stale handles.
#[derive(Default)]
struct LegacyColumn {
    sparse: Vec<Option<NonZeroUsize>>,
    dense: Vec<(Entity, u32)>,
}

impl LegacyColumn {
    fn position(&self, entity: Entity) -> Option<usize> {
        let position = self.sparse.get(entity.index() as usize)?.as_ref()?.get() - 1;
        (self.dense.get(position)?.0 == entity).then_some(position)
    }

    fn get(&self, entity: Entity) -> Option<u32> {
        Some(self.dense[self.position(entity)?].1)
    }

    fn insert(&mut self, entity: Entity, value: u32) {
        let index = entity.index() as usize;
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, None);
        }
        assert!(self.sparse[index].is_none());
        self.sparse[index] = NonZeroUsize::new(self.dense.len() + 1);
        self.dense.push((entity, value));
    }

    fn despawn(&mut self, entity: Entity) {
        if let Some(position) = self.position(entity) {
            self.sparse[entity.index() as usize] = None;
            self.dense.swap_remove(position);
            if let Some(&(moved, _)) = self.dense.get(position) {
                self.sparse[moved.index() as usize] = NonZeroUsize::new(position + 1);
            }
        }
    }
}

fn populate_column<const ID: usize>(world: &mut World, rows: &[(Entity, u8, u16)]) -> LegacyColumn {
    let mut legacy = LegacyColumn::default();
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
            legacy.insert(entity, value);
        }
    }
    legacy
}

fn assert_sparse_join(world: &mut World, handles: &[Entity], legacy: &[LegacyColumn; 4]) {
    let rare_rows = legacy[0].dense.len();
    let key = |entity: Entity| (entity.index(), entity.generation());

    macro_rules! assert_join {
        ($($id:literal => $value:ident),+ $(,)?) => {{
            let mut expected: Vec<_> = handles.iter().filter_map(|&entity| {
                Some((entity, ($(legacy[$id].get(entity)?,)+)))
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

    let mut expected: Vec<_> = handles
        .iter()
        .filter_map(|&entity| {
            if legacy[0].get(entity).is_some() {
                return None;
            }
            legacy[3].get(entity)?;
            Some((entity, legacy[1].get(entity)?))
        })
        .collect();
    let mut filtered = Vec::new();
    world
        .query_filtered_mut::<&Sample<1>, (With<Sample<3>>, Without<Sample<0>>)>()
        .unwrap()
        .for_each(|entity, sample| filtered.push((entity, sample.0)));
    expected.sort_unstable_by_key(|&(entity, _)| key(entity));
    filtered.sort_unstable_by_key(|&(entity, _)| key(entity));
    assert_eq!(filtered, expected);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: if cfg!(miri) { 6 } else { 256 },
        rng_seed: RngSeed::Fixed(0x4d59_5448_4452_4956),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn queries_match_legacy_columns_across_deferred_despawns(
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

        let mut legacy = [
            populate_column::<0>(&mut world, &rows),
            populate_column::<1>(&mut world, &rows),
            populate_column::<2>(&mut world, &rows),
            populate_column::<3>(&mut world, &rows),
        ];
        let mut handles: Vec<_> = rows.iter().map(|&(entity, _, _)| entity).collect();

        let mut commands = CommandBuffer::new();
        for &entity in &dead {
            commands.despawn(entity);
        }
        let pending = commands.spawn();
        commands.insert(&pending, Sample::<1>(111));
        commands.insert(&pending, Sample::<2>(222));
        commands.insert(&pending, Sample::<3>(333));
        assert_sparse_join(&mut world, &handles, &legacy);

        let report = commands.flush(&mut world);
        assert!(report.failures().is_empty());
        assert!(dead.iter().all(|&entity| !world.is_alive(entity)));
        let reused = report.resolve(&pending).unwrap();
        let previous = dead.last().unwrap();
        assert_eq!(reused.index(), previous.index());
        assert_ne!(reused.generation(), previous.generation());
        handles.push(reused);
        for column in &mut legacy {
            for &entity in &dead {
                column.despawn(entity);
            }
        }
        legacy[1].insert(reused, 111);
        legacy[2].insert(reused, 222);
        legacy[3].insert(reused, 333);
        assert_sparse_join(&mut world, &handles, &legacy);
    }
}
