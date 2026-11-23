//! Reflection of components through the world's registry.

#![cfg(feature = "reflect")]

use std::any::{TypeId, type_name};
use std::panic::{AssertUnwindSafe, catch_unwind};

use mythraan_ecs::Changed;
use mythraan_ecs::{ReflectComponentError, World};
use mythraan_reflect::Reflect;

#[derive(Reflect)]
struct Motion {
    speed: u32,
}

#[test]
fn replacing_a_field_preserves_type_safety_and_marks_only_successful_writes() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, Motion { speed: 7 }).unwrap();
    world.register_type::<Motion>().unwrap();
    world.advance_change_tick().unwrap();
    let type_id = TypeId::of::<Motion>();
    assert_eq!(
        world.set_reflected_field(entity, type_id, "speed", -1_i32),
        Err(ReflectComponentError::TypeMismatch {
            expected: TypeId::of::<u32>(),
            actual: TypeId::of::<i32>(),
        })
    );
    assert_eq!(
        world.set_reflected_field(entity, type_id, "missing", 8_u32),
        Err(ReflectComponentError::UnknownField(String::from("missing")))
    );
    assert_eq!(world.get::<Motion>(entity).unwrap().speed, 7);
    world
        .query_filtered_mut::<&Motion, Changed<Motion>>()
        .unwrap()
        .for_each(|_, _| panic!("a rejected edit must leave the tick unchanged"));
    world
        .set_reflected_field(entity, type_id, "speed", 31_u32)
        .unwrap();
    let mut changed = Vec::new();
    world
        .query_filtered_mut::<&Motion, Changed<Motion>>()
        .unwrap()
        .for_each(|matched, motion| changed.push((matched, motion.speed)));
    assert_eq!(changed, [(entity, 31)]);
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

#[test]
fn a_stale_entity_cannot_edit_its_recycled_slot() {
    let mut world = World::new();
    world.register_type::<Motion>().unwrap();
    let stale = world.spawn().unwrap();
    world.insert(stale, Motion { speed: 3 }).unwrap();
    assert!(world.despawn(stale));
    let current = world.spawn().unwrap();
    assert_eq!(stale.index(), current.index());
    world.insert(current, Motion { speed: 5 }).unwrap();
    assert_eq!(
        world.set_reflected_field(stale, TypeId::of::<Motion>(), "speed", 11_u32),
        Err(ReflectComponentError::MissingComponent(stale))
    );
    assert_eq!(world.get::<Motion>(current).unwrap().speed, 5);
}

struct DropBomb(bool);

impl Drop for DropBomb {
    fn drop(&mut self) {
        assert!(!self.0, "old value panicked during drop");
    }
}

#[derive(Reflect)]
struct Slot {
    value: DropBomb,
}

#[test]
fn replacing_a_field_publishes_the_new_value_before_dropping_the_old_one() {
    let mut world = World::new();
    world.register_type::<Slot>().unwrap();
    let entity = world.spawn().unwrap();
    world
        .insert(
            entity,
            Slot {
                value: DropBomb(true),
            },
        )
        .unwrap();
    world.advance_change_tick().unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            world.set_reflected_field(entity, TypeId::of::<Slot>(), "value", DropBomb(false))
        }))
        .is_err()
    );
    let mut seen = Vec::new();
    world
        .query_filtered_mut::<&Slot, Changed<Slot>>()
        .unwrap()
        .for_each(|matched, slot| seen.push((matched, slot.value.0)));
    assert_eq!(seen, [(entity, false)]);
}
