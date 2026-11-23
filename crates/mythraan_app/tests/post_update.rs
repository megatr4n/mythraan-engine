//! Transform propagation after post-update commands and before render.

use std::time::Duration;

use mythraan_app::prelude::{App, Stage};
use mythraan_ecs::prelude::{Commands, Query, SystemAccess};
use mythraan_math::glam::Vec3;
use mythraan_math::transform::{GlobalTransform, Transform};

#[test]
fn render_sees_world_transforms_from_post_update_commands() {
    let mut app = App::new();
    let parent = app.world_mut().spawn().unwrap();
    let child = app.world_mut().spawn().unwrap();
    app.world_mut().set_parent(child, Some(parent)).unwrap();
    app.world_mut().insert(child, Transform::IDENTITY).unwrap();

    app.schedule_mut(Stage::PostUpdate).unwrap().add_system(
        SystemAccess::of::<Commands<'_>>(),
        move |_, commands| {
            commands.unwrap().insert(
                parent,
                Transform {
                    translation: Vec3::X,
                    ..Transform::IDENTITY
                },
            );
            Ok(())
        },
    );
    app.schedule_mut(Stage::Render).unwrap().add_system(
        SystemAccess::of::<Query<'_, &GlobalTransform>>(),
        move |scope, _| {
            let transform = scope.components.get::<GlobalTransform>(child)?.unwrap();
            assert_eq!(transform.0.transform_point3(Vec3::ZERO), Vec3::X);
            Ok(())
        },
    );
    app.update(Duration::ZERO).unwrap();
    assert_eq!(
        app.world()
            .get::<GlobalTransform>(child)
            .unwrap()
            .0
            .transform_point3(Vec3::ZERO),
        Vec3::X
    );
}
