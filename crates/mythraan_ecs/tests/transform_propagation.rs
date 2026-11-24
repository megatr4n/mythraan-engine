//! World-space transforms through the public hierarchy API.

use mythraan_ecs::prelude::{Changed, CommandBuffer, Entity, World};
use mythraan_math::glam::{Quat, Vec3};
use mythraan_math::transform::{GlobalTransform, Transform};

fn origin(world: &World, entity: Entity) -> Vec3 {
    world
        .get::<GlobalTransform>(entity)
        .unwrap()
        .0
        .transform_point3(Vec3::ZERO)
}

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
    assert_eq!(origin(&world, leaf), Vec3::X * 21.0);
    let mut changed = Vec::new();
    world
        .query_filtered_mut::<&GlobalTransform, Changed<GlobalTransform>>()
        .unwrap()
        .for_each(|entity, _| changed.push(entity));
    assert_eq!(changed, [leaf]);

    world.set_parent(group, None).unwrap();
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, leaf), Vec3::X);
}

#[test]
fn mutable_queries_and_flushed_commands_invalidate_cached_transforms() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, Transform::IDENTITY).unwrap();
    world.propagate_transforms().unwrap();

    world
        .query_mut::<&mut Transform>()
        .unwrap()
        .for_each(|_, transform| {
            transform.translation = Vec3::Z;
        });
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, entity), Vec3::Z);

    let mut commands = CommandBuffer::new();
    commands.insert(
        entity,
        Transform {
            translation: Vec3::Y,
            ..Transform::IDENTITY
        },
    );
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, entity), Vec3::Z);

    assert!(commands.flush(&mut world).failures().is_empty());
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, entity), Vec3::Y);
}

#[test]
fn removing_a_local_transform_drops_its_cache_and_updates_descendants() {
    let mut world = World::new();
    let parent = world.spawn().unwrap();
    let child = world.spawn().unwrap();
    world.set_parent(child, Some(parent)).unwrap();
    world
        .insert(
            parent,
            Transform {
                translation: Vec3::Z,
                ..Transform::IDENTITY
            },
        )
        .unwrap();
    world
        .insert(
            child,
            Transform {
                translation: Vec3::Y,
                ..Transform::IDENTITY
            },
        )
        .unwrap();
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, child), Vec3::Y + Vec3::Z);

    assert!(world.remove::<Transform>(parent).is_some());
    assert!(world.get::<GlobalTransform>(parent).is_none());
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, child), Vec3::Y);
    assert!(world.get::<GlobalTransform>(parent).is_none());
}

#[test]
fn propagation_repairs_edited_or_removed_derived_values() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world
        .insert(
            entity,
            Transform {
                translation: Vec3::X,
                ..Transform::IDENTITY
            },
        )
        .unwrap();
    world.propagate_transforms().unwrap();

    world
        .query_mut::<&mut GlobalTransform>()
        .unwrap()
        .for_each(|_, global| {
            *global = GlobalTransform::IDENTITY;
        });
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, entity), Vec3::X);

    assert!(world.remove::<GlobalTransform>(entity).is_some());
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, entity), Vec3::X);
}

#[test]
fn propagation_handles_a_deep_chain_from_root_to_leaf() {
    const DEPTH: u16 = if cfg!(miri) { 192 } else { 16_384 };
    let mut world = World::new();
    let leaf = world.spawn().unwrap();
    let mut root = leaf;
    let mut chain = vec![leaf];
    for _ in 1..DEPTH {
        let parent = world.spawn().unwrap();
        world.set_parent(root, Some(parent)).unwrap();
        chain.push(parent);
        root = parent;
    }

    world.propagate_transforms().unwrap();
    assert_eq!(world.query::<&GlobalTransform>().count(), 0);
    for &entity in &chain {
        world
            .insert(
                entity,
                Transform {
                    translation: Vec3::X,
                    ..Transform::IDENTITY
                },
            )
            .unwrap();
    }

    world.advance_change_tick().unwrap();
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, leaf), Vec3::X * f32::from(DEPTH));
    world.get_mut::<Transform>(root).unwrap().translation = Vec3::X * 2.0;
    world.propagate_transforms().unwrap();
    assert_eq!(origin(&world, leaf), Vec3::X * (f32::from(DEPTH) + 1.0));

    let branch_len = DEPTH / 2;
    let branch = &chain[..usize::from(branch_len)];
    let branch_root = *branch.last().unwrap();
    world.set_parent(branch_root, Some(root)).unwrap();
    world.propagate_transforms().unwrap();
    assert_eq!(
        origin(&world, leaf),
        Vec3::X * (f32::from(branch_len) + 2.0)
    );

    assert!(world.despawn(branch_root));
    world.propagate_transforms().unwrap();
    assert!(branch.iter().all(|&entity| !world.is_alive(entity)));
    let remaining = usize::from(DEPTH - branch_len);
    assert_eq!(world.query::<&Transform>().count(), remaining);
    assert_eq!(world.query::<&GlobalTransform>().count(), remaining);
    assert_eq!(origin(&world, root), Vec3::X * 2.0);
}
