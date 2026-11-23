//! Ordering and conditional execution for schedules.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use mythraan_ecs::ScheduleConfigError;
use mythraan_ecs::prelude::{
    Commands, Query, ResMut, Schedule, Stages, SystemAccess, SystemOrder, World,
};

#[test]
fn generic_stages_keep_insertion_order_and_flush_between_schedules() {
    #[derive(Debug, PartialEq, Eq)]
    enum Stage {
        Prepare,
        Read,
        Missing,
    }

    let mut world = World::new();
    let entity = world.spawn().unwrap();
    let mut stages = Stages::new();
    assert!(stages.insert(Stage::Prepare, Schedule::new()).is_none());
    assert!(stages.insert(Stage::Read, Schedule::new()).is_none());
    assert!(stages.get(&Stage::Missing).is_none());
    let mut prepare = Schedule::new();
    prepare.add_system(SystemAccess::of::<Commands<'_>>(), move |_, commands| {
        commands.unwrap().insert(entity, true);
        Ok(())
    });
    assert!(stages.insert(Stage::Prepare, prepare).is_some());
    stages.get_mut(&Stage::Read).unwrap().add_system(
        SystemAccess::of::<Query<'_, &bool>>(),
        move |scope, _| {
            assert_eq!(scope.components.get::<bool>(entity)?, Some(&true));
            Ok(())
        },
    );
    let mut visited = Vec::new();
    for (label, schedule) in stages.iter_mut() {
        assert!(
            schedule
                .run(&mut world)
                .unwrap()
                .iter()
                .all(|run| run.report.failures().is_empty())
        );
        visited.push(label);
    }
    assert_eq!(visited, [&Stage::Prepare, &Stage::Read]);
}

#[test]
fn explicit_order_overrides_conflict_defaults_and_invalid_order_is_atomic() {
    let mut world = World::new();
    world.resources_mut().insert(Vec::<u8>::new());
    let mut schedule = Schedule::new();
    let access = SystemAccess::of::<ResMut<'_, Vec<u8>>>();
    let first = schedule.add_system(access.clone(), |mut scope, _| {
        scope.resources.get_mut::<Vec<u8>>()?.unwrap().push(1);
        Ok(())
    });
    let second = schedule.add_system(access, |mut scope, _| {
        scope.resources.get_mut::<Vec<u8>>()?.unwrap().push(2);
        Ok(())
    });
    let mut order = SystemOrder::new();
    schedule
        .configure_order(order.after(first, second))
        .unwrap();
    assert_eq!(
        schedule.configure_order(SystemOrder::new().before(second, usize::MAX)),
        Err(ScheduleConfigError::UnknownSystem(usize::MAX))
    );
    assert_eq!(
        schedule.configure_order(order.before(first, second)),
        Err(ScheduleConfigError::Cycle)
    );
    assert!(schedule.run(&mut world).unwrap().is_empty());
    assert_eq!(world.resources().get::<Vec<u8>>().unwrap(), &[2, 1]);
}

#[test]
fn named_sets_order_independent_systems_and_accept_overlapping_memberships() {
    let mut schedule = Schedule::new();
    let completed = Arc::new(AtomicUsize::new(0));
    let progress = Arc::clone(&completed);
    let reader = schedule.add_system(SystemAccess::of::<()>(), move |_, _| {
        assert_eq!(progress.load(Ordering::SeqCst), 3);
        Ok(())
    });
    let first = Arc::clone(&completed);
    let write_first = schedule.add_system(SystemAccess::of::<()>(), move |_, _| {
        first.fetch_or(1, Ordering::SeqCst);
        Ok(())
    });
    let second = Arc::clone(&completed);
    let write_second = schedule.add_system(SystemAccess::of::<()>(), move |_, _| {
        second.fetch_or(2, Ordering::SeqCst);
        Ok(())
    });

    let mut order = SystemOrder::new();
    order
        .set("writers", [write_first, write_second])
        .set("first", [write_first])
        .set("readers", [reader])
        .before("writers", "readers")
        .after(reader, "first");
    schedule.configure_order(&order).unwrap();
    assert!(schedule.run(&mut World::new()).unwrap().is_empty());
}

#[test]
fn invalid_set_references_and_self_dependencies_are_rejected() {
    let mut schedule = Schedule::new();
    let system = schedule.add_system(SystemAccess::of::<()>(), |_, _| Ok(()));
    assert_eq!(
        schedule.configure_order(SystemOrder::new().before("missing", system)),
        Err(ScheduleConfigError::UnknownSet("missing"))
    );
    assert_eq!(
        schedule.configure_order(SystemOrder::new().set("invalid", [usize::MAX])),
        Err(ScheduleConfigError::UnknownSystem(usize::MAX))
    );
    assert_eq!(
        schedule.configure_order(
            SystemOrder::new()
                .set("including", [system])
                .before("including", system)
        ),
        Err(ScheduleConfigError::Cycle)
    );
}
