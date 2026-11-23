//! Reflection of components through the world's registry.

#![cfg(feature = "reflect")]

use std::any::{TypeId, type_name};

use mythraan_ecs::{ReflectComponentError, World};
use mythraan_reflect::Reflect;

#[derive(Reflect)]
struct Motion {
    speed: u32,
}

#[test]
fn registered_metadata_resolves_a_field_on_an_existing_component() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, Motion { speed: 7 }).unwrap();
    assert!(matches!(
        world.reflected_field(entity, TypeId::of::<Motion>(), "speed"),
        Err(ReflectComponentError::UnregisteredType(_))
    ));
    world.register_type::<Motion>().unwrap();
    world.register_type::<Motion>().unwrap();
    let type_id = world
        .type_registry()
        .get_by_name(type_name::<Motion>())
        .unwrap()
        .type_id();
    let speed = world.reflected_field(entity, type_id, "speed").unwrap();
    assert_eq!(speed.downcast_ref::<u32>(), Some(&7));
    assert!(matches!(
        world.reflected_field(entity, type_id, "missing"),
        Err(ReflectComponentError::UnknownField(name)) if name == "missing"
    ));
    assert!(world.despawn(entity));
    assert!(matches!(
        world.reflected_field(entity, type_id, "speed"),
        Err(ReflectComponentError::MissingComponent(stale)) if stale == entity
    ));
}
