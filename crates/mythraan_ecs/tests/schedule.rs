//! Conflict-aware execution through the public schedule API.

use std::any::TypeId;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use mythraan_ecs::prelude::{Commands, Query, Res, ResMut, Schedule, SystemAccess, World};
use mythraan_ecs::{CommandError, ScheduleError, ScopeError, StorageKind};

#[test]
fn conflicting_systems_see_writes_in_registration_order() {
    let mut world = World::new();
    world.resources_mut().insert(1_u32);
    let mut schedule = Schedule::new();
    schedule.add_system(
        SystemAccess::of::<ResMut<'_, u32>>(),
        |mut scope, commands| {
            assert!(commands.is_none());
            *scope.resources.get_mut::<u32>()?.unwrap() += 2;
            Ok(())
        },
    );
    schedule.add_system(SystemAccess::of::<Res<'_, u32>>(), |scope, _| {
        assert_eq!(*scope.resources.get::<u32>()?.unwrap(), 3);
        Ok(())
    });
    schedule.add_system(SystemAccess::of::<ResMut<'_, u32>>(), |mut scope, _| {
        *scope.resources.get_mut::<u32>()?.unwrap() *= 2;
        Ok(())
    });

    assert!(schedule.run(&mut world).unwrap().is_empty());
    assert_eq!(world.resources().get::<u32>(), Some(&6));
}

#[test]
fn commands_flush_once_in_registration_order_even_across_batches() {
    #[derive(Debug, PartialEq, Eq)]
    struct Tag(u32);

    let mut world = World::new();
    let target = world.spawn().unwrap();
    world.insert(target, Tag(0)).unwrap();
    world.resources_mut().insert(false);
    let placeholder = Arc::new(Mutex::new(None));
    let mut schedule = Schedule::new();
    schedule.add_system(
        SystemAccess::of::<(ResMut<'_, bool>, Commands<'_>)>(),
        move |mut scope, commands| {
            *scope.resources.get_mut::<bool>()?.unwrap() = true;
            let mut commands = commands.unwrap();
            commands.insert(target, Tag(1));
            commands.insert(target, Tag(2));
            Ok(())
        },
    );
    schedule.add_system(
        SystemAccess::of::<(Res<'_, bool>, Query<'_, &Tag>, Commands<'_>)>(),
        move |scope, commands| {
            assert!(*scope.resources.get::<bool>()?.unwrap());
            assert_eq!(scope.components.get::<Tag>(target)?, Some(&Tag(0)));
            commands.unwrap().insert(target, Tag(3));
            Ok(())
        },
    );
    let output = Arc::clone(&placeholder);
    schedule.add_system(SystemAccess::of::<Commands<'_>>(), move |_, commands| {
        let mut commands = commands.unwrap();
        commands.insert(target, Tag(4));
        let spawned = commands.spawn();
        commands.insert(&spawned, Tag(5));
        *output.lock().unwrap() = Some(spawned);
        Ok(())
    });

    let reports = schedule.run(&mut world).unwrap();
    assert_eq!(
        reports.iter().map(|entry| entry.system).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(
        reports
            .iter()
            .all(|entry| entry.report.failures().is_empty())
    );
    assert_eq!(world.get::<Tag>(target), Some(&Tag(4)));
    let placeholder = placeholder.lock().unwrap();
    let spawned = reports[2]
        .report
        .resolve(placeholder.as_ref().unwrap())
        .unwrap();
    assert_eq!(world.get::<Tag>(spawned), Some(&Tag(5)));
}

#[test]
fn a_failed_command_reports_its_system_and_does_not_prevent_later_commands() {
    let mut world = World::new();
    let stale = world.spawn().unwrap();
    assert!(world.despawn(stale));
    let live = world.spawn().unwrap();
    let mut schedule = Schedule::new();
    schedule.add_system(SystemAccess::of::<Commands<'_>>(), move |_, commands| {
        let mut commands = commands.unwrap();
        commands.insert(stale, 1_u32);
        commands.insert(live, 2_u32);
        Ok(())
    });
    schedule.add_system(SystemAccess::of::<Commands<'_>>(), move |_, commands| {
        commands.unwrap().insert(live, true);
        Ok(())
    });

    let reports = schedule.run(&mut world).unwrap();
    let [first, second] = reports.as_slice() else {
        panic!("expected reports for two command queues")
    };
    assert_eq!(first.system, 0);
    let [failure] = first.report.failures() else {
        panic!("expected one command failure")
    };
    assert_eq!(failure.index, 0);
    assert!(matches!(failure.error, CommandError::Insert(_)));
    assert_eq!(second.system, 1);
    assert!(second.report.failures().is_empty());
    assert_eq!(world.get::<u32>(live), Some(&2));
    assert_eq!(world.get::<bool>(live), Some(&true));
}

#[test]
fn independent_systems_overlap_and_a_dependent_system_waits_for_both() {
    let mut world = World::new();
    world.resources_mut().insert(1_u32);
    world.resources_mut().insert(false);
    let rendezvous = {
        let ready = Arc::new((Mutex::new(0), Condvar::new()));
        move || {
            let (count, signal) = &*ready;
            let mut count = count.lock().unwrap();
            *count += 1;
            signal.notify_all();
            let (count, _) = signal
                .wait_timeout_while(count, Duration::from_secs(5), |count| *count < 2)
                .unwrap();
            assert_eq!(*count, 2, "the two compatible systems must overlap");
        }
    };
    let first = rendezvous.clone();
    let mut schedule = Schedule::new();
    schedule.add_system(
        SystemAccess::of::<ResMut<'_, u32>>(),
        move |mut scope, _| {
            let mut number = scope.resources.get_mut::<u32>()?.unwrap();
            first();
            *number += 5;
            Ok(())
        },
    );
    schedule.add_system(
        SystemAccess::of::<ResMut<'_, bool>>(),
        move |mut scope, _| {
            let mut flag = scope.resources.get_mut::<bool>()?.unwrap();
            rendezvous();
            *flag = true;
            Ok(())
        },
    );
    schedule.add_system(
        SystemAccess::of::<(Res<'_, u32>, Res<'_, bool>)>(),
        |scope, _| {
            assert_eq!(*scope.resources.get::<u32>()?.unwrap(), 6);
            assert!(*scope.resources.get::<bool>()?.unwrap());
            Ok(())
        },
    );

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    assert!(
        pool.install(|| schedule.run(&mut world))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn repeated_runs_keep_callback_state_and_new_systems_rebuild_the_batches() {
    let mut world = World::new();
    world.resources_mut().insert(1_u32);
    world.resources_mut().insert(false);
    let mut schedule = Schedule::new();
    let mut increment = 1;
    schedule.add_system(
        SystemAccess::of::<ResMut<'_, u32>>(),
        move |mut scope, _| {
            *scope.resources.get_mut::<u32>()?.unwrap() += increment;
            increment += 1;
            Ok(())
        },
    );
    schedule.add_system(SystemAccess::of::<ResMut<'_, u32>>(), |mut scope, _| {
        *scope.resources.get_mut::<u32>()?.unwrap() *= 2;
        Ok(())
    });
    schedule.add_system(SystemAccess::of::<ResMut<'_, bool>>(), |mut scope, _| {
        let mut flag = scope.resources.get_mut::<bool>()?.unwrap();
        *flag = !*flag;
        Ok(())
    });
    assert!(schedule.run(&mut world).unwrap().is_empty());
    assert_eq!(world.resources().get::<u32>(), Some(&4));
    assert!(schedule.run(&mut world).unwrap().is_empty());
    assert_eq!(world.resources().get::<u32>(), Some(&12));

    let reader = schedule.add_system(
        SystemAccess::of::<(Res<'_, u32>, Res<'_, bool>)>(),
        |scope, _| {
            assert_eq!(*scope.resources.get::<u32>()?.unwrap(), 30);
            assert!(*scope.resources.get::<bool>()?.unwrap());
            Ok(())
        },
    );
    assert_eq!(reader, 3);
    assert!(schedule.run(&mut world).unwrap().is_empty());
}

#[test]
fn a_system_error_discards_pending_commands_without_rolling_back_direct_writes() {
    let mut world = World::new();
    let target = world.spawn().unwrap();
    world.resources_mut().insert(0_u32);
    let mut schedule = Schedule::new();
    schedule.add_system(
        SystemAccess::of::<(ResMut<'_, u32>, Commands<'_>)>(),
        move |mut scope, commands| {
            let mut count = scope.resources.get_mut::<u32>()?.unwrap();
            *count += 1;
            if *count == 1 {
                commands.unwrap().insert(target, true);
            }
            Ok(())
        },
    );
    let failing = schedule.add_system(SystemAccess::of::<Res<'_, u32>>(), |scope, _| {
        if *scope.resources.get::<u32>()?.unwrap() == 1 {
            scope.resources.get::<bool>()?;
        }
        Ok(())
    });
    schedule.add_system(SystemAccess::of::<Res<'_, u32>>(), |scope, _| {
        if *scope.resources.get::<u32>()?.unwrap() == 1 {
            scope.resources.get::<i32>()?;
        }
        Ok(())
    });

    assert_eq!(
        schedule.run(&mut world).unwrap_err(),
        ScheduleError::System {
            system: failing,
            source: ScopeError::UndeclaredAccess {
                storage: StorageKind::Resource,
                type_id: TypeId::of::<bool>(),
                write: false,
            },
        },
    );
    assert_eq!(world.resources().get::<u32>(), Some(&1));
    assert!(world.get::<bool>(target).is_none());
    assert!(schedule.run(&mut world).unwrap().is_empty());
    assert_eq!(world.resources().get::<u32>(), Some(&2));
    assert!(world.get::<bool>(target).is_none());
}

#[test]
fn a_panicking_system_cannot_replay_its_commands_on_retry() {
    let mut world = World::new();
    let target = world.spawn().unwrap();
    let mut schedule = Schedule::new();
    let mut first = true;
    schedule.add_system(SystemAccess::of::<Commands<'_>>(), move |_, commands| {
        let mut commands = commands.unwrap();
        if std::mem::take(&mut first) {
            commands.insert(target, true);
            panic!("fail after queuing a command");
        }
        commands.insert(target, 7_u32);
        Ok(())
    });
    assert!(catch_unwind(AssertUnwindSafe(|| schedule.run(&mut world))).is_err());
    assert!(world.get::<bool>(target).is_none());
    assert_eq!(schedule.run(&mut world).unwrap().len(), 1);
    assert_eq!(world.get::<u32>(target), Some(&7));
    assert!(world.get::<bool>(target).is_none());
}
