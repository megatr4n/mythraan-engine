//! Standard frame order and application integration.

use mythraan_app::prelude::{App, Plugin, Stage, Time};

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
