//! Error handling at frame and fixed-step boundaries.

use std::time::Duration;

use mythraan_app::prelude::{App, AppError, Stage, Time};
use mythraan_ecs::prelude::{Commands, ResMut, SystemAccess};

#[test]
fn a_failed_fixed_queue_stops_the_frame_and_leaves_other_steps_for_retry() {
    let mut app = App::new();
    let stale = app.world_mut().spawn().unwrap();
    assert!(app.world_mut().despawn(stale));
    let live = app.world_mut().spawn().unwrap();
    app.insert_resource(false);
    let mut should_fail = true;
    app.schedule_mut(Stage::FixedUpdate).unwrap().add_system(
        SystemAccess::of::<Commands<'_>>(),
        move |_, commands| {
            let mut commands = commands.unwrap();
            if std::mem::take(&mut should_fail) {
                commands.insert(stale, true);
            }
            commands.insert(live, 17_u8);
            Ok(())
        },
    );
    app.schedule_mut(Stage::Last).unwrap().add_system(
        SystemAccess::of::<ResMut<'_, bool>>(),
        |mut scope, _| {
            *scope.resources.get_mut::<bool>()?.unwrap() = true;
            Ok(())
        },
    );

    let frame_delta = Duration::from_millis(50);
    let AppError::Commands {
        stage: Stage::FixedUpdate,
        reports,
    } = app.update(frame_delta).unwrap_err()
    else {
        panic!("expected a failed fixed-update command");
    };
    assert_eq!(reports[0].system, 0);
    assert_eq!(reports[0].report.failures().len(), 1);
    assert_eq!(app.world().get::<u8>(live), Some(&17));
    assert_eq!(app.world().resources().get::<bool>(), Some(&false));
    let time = app.world().resources().get::<Time>().unwrap();
    assert_eq!(time.delta(), frame_delta);
    assert_eq!(time.fixed_ticks(), 1);

    app.update(Duration::ZERO).unwrap();
    assert_eq!(app.world().resources().get::<bool>(), Some(&true));
    assert_eq!(
        app.world().resources().get::<Time>().unwrap().fixed_ticks(),
        3
    );
}
