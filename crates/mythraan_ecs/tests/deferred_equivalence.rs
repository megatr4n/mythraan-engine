//! Compare deferred command streams with immediate operations on an identical world.

use std::sync::{Arc, Mutex};

use mythraan_ecs::{CommandBuffer, CommandError, CommandTarget, Entity, World, WorldInsertError};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

#[derive(Clone, Copy, Debug)]
enum Action {
    Spawn,
    Despawn,
    InsertA,
    InsertB,
    RemoveA,
    RemoveB,
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Known(u8),
    Current(u8),
    Previous,
    Foreign,
}

fn command() -> impl Strategy<Value = (Action, Target)> {
    let action = prop::sample::select(vec![
        Action::Spawn,
        Action::Despawn,
        Action::InsertA,
        Action::InsertB,
        Action::RemoveA,
        Action::RemoveB,
    ]);
    let target = prop_oneof![
        3 => any::<u8>().prop_map(Target::Known),
        3 => any::<u8>().prop_map(Target::Current),
        1 => Just(Target::Previous),
        1 => Just(Target::Foreign),
    ];
    (action, target)
}

type Drops = Arc<Mutex<Vec<usize>>>;

struct Payload<const KIND: u8>(usize, Drops);

impl<const KIND: u8> Drop for Payload<KIND> {
    fn drop(&mut self) {
        self.1.lock().unwrap().push(self.0);
    }
}

fn check_batches(batches: Vec<Vec<(Action, Target)>>) {
    let mut queued = World::new();
    let mut direct = World::new();
    let mut commands = CommandBuffer::new();
    let queued_drops = Drops::default();
    let direct_drops = Drops::default();
    let mut serial = 0;
    let foreign = CommandBuffer::new().spawn();
    let mut previous = foreign.clone();
    let first = direct.spawn().unwrap();
    assert_eq!(queued.spawn().unwrap(), first);
    let mut known = vec![first];

    for batch in batches {
        // Seed each batch with a local placeholder so every target choice is valid.
        let mut spawns = vec![(commands.spawn(), direct.spawn().unwrap())];
        let mut expected_failures = Vec::new();
        for (index, (action, target)) in batch.into_iter().enumerate() {
            if matches!(action, Action::Spawn) {
                spawns.push((commands.spawn(), direct.spawn().unwrap()));
                continue;
            }
            let (target, entity): (CommandTarget, Option<Entity>) = match target {
                Target::Known(key) => {
                    let id = known[usize::from(key) % known.len()];
                    (id.into(), Some(id))
                }
                Target::Current(key) => {
                    let (placeholder, id) = &spawns[usize::from(key) % spawns.len()];
                    (placeholder.into(), Some(*id))
                }
                Target::Previous => ((&previous).into(), None),
                Target::Foreign => ((&foreign).into(), None),
            };
            let inserts = matches!(action, Action::InsertA | Action::InsertB);
            if entity.is_none() || (inserts && entity.is_some_and(|e| !direct.is_alive(e))) {
                expected_failures.push((index + 1, entity));
            }
            match action {
                Action::Spawn => unreachable!(),
                Action::Despawn => {
                    commands.despawn(target);
                    if let Some(id) = entity {
                        direct.despawn(id);
                    }
                }
                Action::InsertA => {
                    commands.insert(target, Payload::<0>(serial, queued_drops.clone()));
                    let value = Payload::<0>(serial, direct_drops.clone());
                    drop(entity.map(|id| direct.insert(id, value)));
                }
                Action::InsertB => {
                    commands.insert(target, Payload::<1>(serial, queued_drops.clone()));
                    let value = Payload::<1>(serial, direct_drops.clone());
                    drop(entity.map(|id| direct.insert(id, value)));
                }
                Action::RemoveA => {
                    commands.remove::<Payload<0>>(target);
                    drop(entity.and_then(|id| direct.remove::<Payload<0>>(id)));
                }
                Action::RemoveB => {
                    commands.remove::<Payload<1>>(target);
                    drop(entity.and_then(|id| direct.remove::<Payload<1>>(id)));
                }
            }
            serial += 1;
        }

        let report = commands.flush(&mut queued);
        let failures: Vec<_> = report
            .failures()
            .iter()
            .map(|f| {
                let dead = match f.error {
                    CommandError::ForeignPlaceholder => None,
                    CommandError::Insert(WorldInsertError::EntityNotAlive { entity }) => {
                        Some(entity)
                    }
                    ref error => panic!("unexpected command error: {error}"),
                };
                (f.index, dead)
            })
            .collect();
        assert_eq!(failures, expected_failures);
        assert_eq!(report.resolve(&previous), None);
        assert_eq!(report.resolve(&foreign), None);
        previous = spawns[0].0.clone();
        for (placeholder, entity) in spawns {
            assert_eq!(report.resolve(&placeholder), Some(entity));
            known.push(entity);
        }
        for &id in &known {
            assert_eq!(queued.is_alive(id), direct.is_alive(id));
            assert_eq!(
                queued.get::<Payload<0>>(id).map(|v| v.0),
                direct.get::<Payload<0>>(id).map(|v| v.0)
            );
            assert_eq!(
                queued.get::<Payload<1>>(id).map(|v| v.0),
                direct.get::<Payload<1>>(id).map(|v| v.0)
            );
        }
        assert_eq!(*queued_drops.lock().unwrap(), *direct_drops.lock().unwrap());
    }
    drop(queued);
    drop(direct);
    assert_eq!(*queued_drops.lock().unwrap(), *direct_drops.lock().unwrap());
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: if cfg!(miri) { 4 } else { 512 },
        rng_seed: RngSeed::Fixed(0x4d59_5448_4741_5445),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn deferred_matches_immediate_across_batches(
        batches in prop::collection::vec(
            prop::collection::vec(command(), 0..=if cfg!(miri) { 12 } else { 48 }), 2..=4,
        )
    ) {
        check_batches(batches);
    }
}
