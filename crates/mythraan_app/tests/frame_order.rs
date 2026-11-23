//! Standard frame order and application integration.

use mythraan_app::prelude::{App, Plugin, Stage, Time};
use mythraan_ecs::prelude::{Res, ResMut, SystemAccess};

#[test]
fn stage_order_follows_the_frame_contract() {
    assert_eq!(
        Stage::ORDER,
        [
            Stage::First,
            Stage::PreUpdate,
            Stage::FixedUpdate,
            Stage::Update,
            Stage::PostUpdate,
            Stage::Render,
            Stage::Last,
        ]
    );
}

struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(7_u32);
    }
}

#[test]
fn plugins_build_immediately_and_the_app_exposes_each_schedule() {
    let mut app = App::new();
    app.add_plugin(ExamplePlugin).insert_resource(true);
    assert_eq!(app.world().resources().get::<u32>(), Some(&7));
    assert_eq!(app.world().resources().get::<bool>(), Some(&true));
    assert_eq!(
        app.world().resources().get::<Time>(),
        Some(&Time::default())
    );
    assert!(
        Stage::ORDER
            .into_iter()
            .all(|stage| app.schedule_mut(stage).is_some())
    );
}

#[derive(Default)]
struct FrameTrace(Vec<(u64, Stage)>);

struct TracePlugin;

impl Plugin for TracePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FrameTrace::default());
        for stage in Stage::ORDER {
            app.schedule_mut(stage).unwrap().add_system(
                SystemAccess::of::<(Res<'_, Time>, ResMut<'_, FrameTrace>)>(),
                move |mut scope, _| {
                    let frame = scope.resources.get::<Time>()?.unwrap().frame();
                    scope
                        .resources
                        .get_mut::<FrameTrace>()?
                        .unwrap()
                        .0
                        .push((frame, stage));
                    Ok(())
                },
            );
        }
    }
}

#[test]
fn a_plugin_registers_and_runs_every_stage_in_each_headless_frame() {
    let mut app = App::new();
    app.add_plugin(TracePlugin);
    app.run_headless(0, Time::FIXED_STEP).unwrap();
    assert!(
        app.world()
            .resources()
            .get::<FrameTrace>()
            .unwrap()
            .0
            .is_empty()
    );
    app.run_headless(2, Time::FIXED_STEP).unwrap();
    let expected = (1..=2).flat_map(|frame| Stage::ORDER.map(|stage| (frame, stage)));
    let trace = app.world().resources().get::<FrameTrace>().unwrap();
    assert!(trace.0.iter().copied().eq(expected));
    let time = app.world().resources().get::<Time>().unwrap();
    assert_eq!(time.frame(), 2);
    assert_eq!(time.fixed_ticks(), 2);
    assert_eq!(time.elapsed(), Time::FIXED_STEP * 2);
}
