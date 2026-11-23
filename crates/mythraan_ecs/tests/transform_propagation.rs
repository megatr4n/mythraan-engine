//! World-space transforms through the public hierarchy API.

use mythraan_ecs::prelude::{Changed, World};
use mythraan_math::glam::{Quat, Vec3};
use mythraan_math::transform::{GlobalTransform, Transform};

#[test]
fn propagation_composes_parent_space_before_children() {
    let mut world = World::new();
    let root = world.spawn().unwrap();
    let group = world.spawn().unwrap();
    let child = world.spawn().unwrap();
    world.set_parent(group, Some(root)).unwrap();
    world.set_parent(child, Some(group)).unwrap();
    let root_transform = Transform {
        translation: Vec3::new(4.0, 5.0, 6.0),
        rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        scale: Vec3::new(2.0, 3.0, 4.0),
    };
    let child_transform = Transform {
        translation: Vec3::X,
        ..Transform::IDENTITY
    };
    world.insert(root, root_transform).unwrap();
    world.insert(child, child_transform).unwrap();

    world.propagate_transforms().unwrap();
    let child_global = world.get::<GlobalTransform>(child).unwrap();
    assert!(
        child_global
            .0
            .transform_point3(Vec3::ZERO)
            .abs_diff_eq(Vec3::new(4.0, 7.0, 6.0), 1.0e-5)
    );
    assert_eq!(
        world.get::<GlobalTransform>(root),
        Some(&root_transform.into())
    );
    assert!(world.get::<GlobalTransform>(group).is_none());
}

#[test]
fn clean_passes_and_unaffected_transforms_preserve_change_ticks() {
    let mut world = World::new();
    let root = world.spawn().unwrap();
    let child = world.spawn().unwrap();
    let unrelated = world.spawn().unwrap();
    world.set_parent(child, Some(root)).unwrap();
    for entity in [root, child, unrelated] {
        world.insert(entity, Transform::IDENTITY).unwrap();
    }
    world.propagate_transforms().unwrap();

    world.advance_change_tick().unwrap();
    world.propagate_transforms().unwrap();
    world
        .query_filtered_mut::<&GlobalTransform, Changed<GlobalTransform>>()
        .unwrap()
        .for_each(|_, _| panic!("clean pass rewrote the cache"));

    world.get_mut::<Transform>(root).unwrap().translation = Vec3::X;
    world.propagate_transforms().unwrap();
    let mut matches = Vec::new();
    world
        .query_filtered_mut::<&GlobalTransform, Changed<GlobalTransform>>()
        .unwrap()
        .for_each(|entity, _| matches.push(entity));
    assert_eq!(matches.len(), 2);
    assert!(matches.contains(&root) && matches.contains(&child));

    world.get_mut::<Transform>(root).unwrap().translation = Vec3::Y;
    world.propagate_transforms().unwrap();
    assert_eq!(
        world
            .get::<GlobalTransform>(child)
            .unwrap()
            .0
            .transform_point3(Vec3::ZERO),
        Vec3::Y
    );
}
