//! Timing resource defaults and fixed-step execution.

use std::time::Duration;

use mythraan_app::prelude::{App, Stage, Time};
use mythraan_ecs::prelude::{Res, ResMut, SystemAccess};

#[test]
fn time_before_the_first_frame_starts_at_zero() {
    let time = Time::default();
    assert_eq!(time.frame(), 0);
    assert_eq!(time.fixed_ticks(), 0);
    assert_eq!(time.elapsed(), Duration::ZERO);
    assert_eq!(time.delta(), Duration::ZERO);
    assert_eq!(time.interpolation_alpha(), 0.0);
    assert_eq!(Time::FIXED_STEP, Duration::from_nanos(16_666_667));
}

#[test]
fn fixed_systems_receive_step_time_and_variable_systems_receive_frame_time() {
    let mut app = App::new();
    app.insert_resource(Vec::<Time>::new());
    let access = SystemAccess::of::<(Res<'_, Time>, ResMut<'_, Vec<Time>>)>();
    for stage in [Stage::FixedUpdate, Stage::Update] {
        app.schedule_mut(stage)
            .unwrap()
            .add_system(access.clone(), |mut scope, _| {
                let time = *scope.resources.get::<Time>()?.unwrap();
                scope.resources.get_mut::<Vec<Time>>()?.unwrap().push(time);
                Ok(())
            });
    }
    let start_tick = app.world().change_tick();
    let delta = Duration::from_millis(50);
    app.update(delta).unwrap();

    let snapshots = app.world().resources().get::<Vec<Time>>().unwrap();
    let [first, second, third, variable] = snapshots.as_slice() else {
        panic!("expected three fixed steps and one variable update");
    };
    for (time, tick) in [(first, 1), (second, 2), (third, 3)] {
        assert_eq!(time.fixed_ticks(), tick);
        assert_eq!(time.delta(), Time::FIXED_STEP);
        assert_eq!(time.frame(), 1);
    }
    assert_eq!(variable.delta(), delta);
    assert_eq!(variable.elapsed(), delta);
    assert_eq!(variable.fixed_ticks(), 3);
    assert_eq!(app.world().change_tick(), start_tick + 1);
}
