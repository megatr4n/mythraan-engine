//! Conflict-aware execution through the public schedule API.

use std::sync::{Arc, Mutex};

use mythraan_ecs::CommandError;
use mythraan_ecs::prelude::{Commands, Query, Res, ResMut, Schedule, SystemAccess, World};

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
