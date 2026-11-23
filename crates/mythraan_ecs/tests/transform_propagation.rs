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

#[test]
fn reparenting_or_detaching_a_group_updates_its_transformed_descendants() {
    let mut world = World::new();
    let left = world.spawn().unwrap();
    let right = world.spawn().unwrap();
    let group = world.spawn().unwrap();
    let leaf = world.spawn().unwrap();
    world.set_parent(group, Some(left)).unwrap();
    world.set_parent(leaf, Some(group)).unwrap();
    for (entity, x) in [(left, 10.0), (right, 20.0), (leaf, 1.0)] {
        world
            .insert(
                entity,
                Transform {
                    translation: Vec3::X * x,
                    ..Transform::IDENTITY
                },
            )
            .unwrap();
    }
    world.propagate_transforms().unwrap();
    world.advance_change_tick().unwrap();

    world.set_parent(group, Some(right)).unwrap();
    world.propagate_transforms().unwrap();
    assert_eq!(
        world
            .get::<GlobalTransform>(leaf)
            .unwrap()
            .0
            .transform_point3(Vec3::ZERO),
        Vec3::X * 21.0
    );
    world
        .query_filtered_mut::<&GlobalTransform, Changed<GlobalTransform>>()
        .unwrap()
        .for_each(|entity, _| assert_eq!(entity, leaf));

    world.set_parent(group, None).unwrap();
    world.propagate_transforms().unwrap();
    assert_eq!(
        world
            .get::<GlobalTransform>(leaf)
            .unwrap()
            .0
            .transform_point3(Vec3::ZERO),
        Vec3::X
    );
}
