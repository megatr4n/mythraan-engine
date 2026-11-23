//! Registers a component through a plugin and edits it through the world registry.

#![cfg(feature = "reflect")]

extern crate self as mythraan_reflect;

use std::any::type_name;

use mythraan_app::prelude::{App, Plugin};
pub use mythraan_ecs::reflection::{FieldInfo, Reflect, ReflectFields};

#[derive(Reflect)]
struct Player {
    name: String,
}

struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Player>().unwrap();
    }
}

#[test]
fn a_plugin_registered_component_can_be_edited_by_name() {
    let mut app = App::new();
    app.add_plugin(PlayerPlugin);
    let world = app.world_mut();
    let player = world.spawn().unwrap();
    world
        .insert(
            player,
            Player {
                name: String::from("Scout"),
            },
        )
        .unwrap();
    let info = world
        .type_registry()
        .get_by_name(type_name::<Player>())
        .copied()
        .unwrap();
    let name_field = info.fields().first().unwrap().name();
    world
        .set_reflected_field(player, info.type_id(), name_field, String::from("Ranger"))
        .unwrap();
    let name = world
        .reflected_field(player, info.type_id(), name_field)
        .unwrap()
        .downcast_ref::<String>()
        .unwrap();
    assert_eq!(name, "Ranger");
    assert_eq!(world.get::<Player>(player).unwrap().name, "Ranger");
}
