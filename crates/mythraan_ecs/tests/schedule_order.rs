//! Ordering and conditional execution for schedules.

use mythraan_ecs::prelude::{Commands, Query, Schedule, Stages, SystemAccess, World};

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
