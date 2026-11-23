//! Safe, isolated storage borrows for a system batch.

use std::any::TypeId;

use mythraan_ecs::prelude::{
    Changed, ConflictGraph, FilteredQuery, Query, QueryMut, Res, ResMut, SystemAccess, With, World,
};
use mythraan_ecs::{QueryError, ScopeError, StorageKind};
use mythraan_math::glam::Vec3;
use mythraan_math::transform::{GlobalTransform, Transform};

#[test]
fn scopes_share_reads_and_allow_disjoint_writes() {
    type First<'world> = (
        QueryMut<'world, (&'world u32, &'world mut i32)>,
        ResMut<'world, i32>,
    );
    type Second<'world> = (
        QueryMut<'world, (&'world u32, &'world mut bool)>,
        ResMut<'world, bool>,
    );
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, 5_u32).unwrap();
    world.insert(entity, 6_i32).unwrap();
    world.insert(entity, true).unwrap();
    world.resources_mut().insert(3_i32);
    world.resources_mut().insert(false);
    let accesses = [
        SystemAccess::of::<First<'_>>(),
        SystemAccess::of::<Second<'_>>(),
    ];

    {
        let mut scopes = world.scope_batch(&accesses).unwrap();
        let [first, second] = scopes.as_mut_slice() else {
            panic!("expected two scopes")
        };
        let shared = first.components.get::<u32>(entity).unwrap().unwrap();
        let other_shared = second.components.get::<u32>(entity).unwrap().unwrap();
        assert!(std::ptr::eq(shared, other_shared));
        let first_component = first.components.get_mut::<i32>(entity).unwrap().unwrap();
        let second_component = second.components.get_mut::<bool>(entity).unwrap().unwrap();
        let mut first_resource = first.resources.get_mut::<i32>().unwrap().unwrap();
        let mut second_resource = second.resources.get_mut::<bool>().unwrap().unwrap();
        *first_component += 2;
        *second_component = false;
        *first_resource += 1;
        *second_resource = true;
    }
    assert_eq!(world.get::<i32>(entity), Some(&8));
    assert_eq!(world.get::<bool>(entity), Some(&false));
    assert_eq!(world.resources().get::<i32>(), Some(&4));
    assert_eq!(world.resources().get::<bool>(), Some(&true));
}

#[test]
fn undeclared_access_and_read_only_writes_return_errors() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    let accesses = [SystemAccess::of::<(Query<'_, &u32>, Res<'_, u32>)>()];
    let mut scopes = world.scope_batch(&accesses).unwrap();
    let scope = scopes.first_mut().unwrap();
    assert_eq!(scope.components.get::<u32>(entity).unwrap(), None);
    assert_eq!(
        scope.components.get_mut::<u32>(entity).err(),
        Some(ScopeError::UndeclaredAccess {
            storage: StorageKind::Component,
            type_id: TypeId::of::<u32>(),
            write: true,
        })
    );
    assert_eq!(
        scope.resources.get::<i32>().err(),
        Some(ScopeError::UndeclaredAccess {
            storage: StorageKind::Resource,
            type_id: TypeId::of::<i32>(),
            write: false,
        })
    );
}

#[test]
fn conflicting_declarations_are_rejected_even_for_missing_storage() {
    let mut world = World::new();
    let read = SystemAccess::of::<Res<'_, u32>>();
    let write = SystemAccess::of::<ResMut<'_, u32>>();
    let expected = Some(ScopeError::BatchConflict {
        storage: StorageKind::Resource,
        type_id: TypeId::of::<u32>(),
    });
    assert_eq!(world.scope_batch([&read, &write]).err(), expected);
    assert_eq!(world.scope_batch([&write, &read]).err(), expected);
    assert_eq!(world.scope_batch([&write, &write]).err(), expected);
}

#[test]
fn scoped_queries_bind_shared_and_exclusive_columns() {
    type Writer<'world> = (
        QueryMut<'world, (&'world u32, &'world mut i32)>,
        ResMut<'world, i32>,
    );
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, 5_u32).unwrap();
    world.insert(entity, 6_i32).unwrap();
    world.resources_mut().insert(7_i32);
    let writer = SystemAccess::of::<Writer<'_>>();
    let reader = SystemAccess::of::<Query<'_, &u32>>();
    {
        let mut scopes = world.scope_batch([&writer, &reader]).unwrap();
        let [writer, reader] = scopes.as_mut_slice() else {
            panic!("expected two scopes")
        };
        let mut bonus = writer.resources.get_mut::<i32>().unwrap().unwrap();
        writer
            .components
            .query_mut::<(&u32, &mut i32)>()
            .unwrap()
            .for_each(|matched, (shared, value)| {
                assert_eq!(matched, entity);
                *value += *shared as i32 + *bonus;
                *bonus += 1;
            });
        reader
            .components
            .query_mut::<&u32>()
            .unwrap()
            .for_each(|matched, value| {
                assert_eq!((matched, *value), (entity, 5));
            });
    }
    assert_eq!(world.get::<i32>(entity), Some(&18));
    assert_eq!(world.resources().get::<i32>(), Some(&8));
}

#[test]
fn scoped_queries_check_optional_and_filter_access_before_binding() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, 7_u32).unwrap();
    let read = SystemAccess::of::<Query<'_, &u32>>();
    let write = SystemAccess::of::<QueryMut<'_, &mut u32>>();
    let filtered = SystemAccess::of::<FilteredQuery<'_, (&u32, Option<&bool>), With<i32>>>();

    let mut scopes = world.scope_batch([&read, &filtered]).unwrap();
    let [read, filtered] = scopes.as_mut_slice() else {
        panic!("expected two scopes")
    };
    let undeclared = Some(ScopeError::UndeclaredAccess {
        storage: StorageKind::Component,
        type_id: TypeId::of::<bool>(),
        write: false,
    });
    assert_eq!(
        read.components.query_mut::<(&u32, Option<&bool>)>().err(),
        undeclared
    );
    assert_eq!(
        read.components
            .query_filtered_mut_since::<&u32, With<bool>>(0)
            .err(),
        undeclared
    );
    assert_eq!(
        read.components.query_mut::<&mut u32>().err(),
        Some(ScopeError::UndeclaredAccess {
            storage: StorageKind::Component,
            type_id: TypeId::of::<u32>(),
            write: true,
        })
    );
    let mut matches = Vec::new();
    filtered
        .components
        .query_mut::<(&u32, Option<&bool>)>()
        .unwrap()
        .for_each(|_, (value, optional)| {
            matches.push((*value, optional.copied()));
        });
    assert_eq!(matches, [(7, None)]);
    filtered
        .components
        .query_filtered_mut_since::<&u32, With<i32>>(0)
        .unwrap()
        .for_each(|_, _| {
            panic!("missing filter storage must produce no matches");
        });

    let mut scopes = world.scope_batch([&write]).unwrap();
    let scope = scopes.first_mut().unwrap();
    assert_eq!(
        scope.components.query_mut::<(&u32, &mut u32)>().err(),
        Some(ScopeError::Query(QueryError { component: "u32" }))
    );
}

#[test]
fn scoped_writes_track_changes_and_invalidate_transform_propagation() {
    let mut world = World::new();
    let root = world.spawn().unwrap();
    let child = world.spawn().unwrap();
    world.insert(root, Transform::IDENTITY).unwrap();
    world.insert(child, Transform::IDENTITY).unwrap();
    world.set_parent(child, Some(root)).unwrap();
    world.propagate_transforms().unwrap();
    let checkpoint = world.change_tick();
    world.advance_change_tick().unwrap();
    let access = SystemAccess::of::<QueryMut<'_, &mut Transform>>();
    {
        let mut scopes = world.scope_batch([&access]).unwrap();
        let scope = scopes.first_mut().unwrap();
        scope
            .components
            .get_mut::<Transform>(root)
            .unwrap()
            .unwrap()
            .translation = Vec3::X;
        let mut changed = Vec::new();
        scope
            .components
            .query_filtered_mut_since::<&Transform, Changed<Transform>>(checkpoint)
            .unwrap()
            .for_each(|entity, _| changed.push(entity));
        assert_eq!(changed, [root]);
    }

    world.propagate_transforms().unwrap();
    let propagated = world.get::<GlobalTransform>(child).unwrap();
    assert_eq!(propagated.0.transform_point3(Vec3::ZERO), Vec3::X);
}

#[test]
fn scoped_queries_only_mark_rows_yielded_by_the_filter() {
    let mut world = World::new();
    let matched = world.spawn().unwrap();
    let skipped = world.spawn().unwrap();
    for entity in [matched, skipped] {
        world.insert(entity, 3_u32).unwrap();
    }
    world.insert(matched, true).unwrap();
    let checkpoint = world.change_tick();
    world.advance_change_tick().unwrap();
    let access = SystemAccess::of::<FilteredQuery<'_, &mut u32, With<bool>>>();
    {
        let mut scopes = world.scope_batch([&access]).unwrap();
        let scope = scopes.first_mut().unwrap();
        scope
            .components
            .query_filtered_mut_since::<&mut u32, With<bool>>(checkpoint)
            .unwrap()
            .for_each(|_, value| *value += 1);
    }
    let mut changed = Vec::new();
    world
        .query_filtered_mut_since::<&u32, Changed<u32>>(checkpoint)
        .unwrap()
        .for_each(|entity, value| changed.push((entity, *value)));
    assert_eq!(changed, [(matched, 4)]);
}

#[test]
fn graph_batches_keep_component_and_resource_borrows_independent() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, 7_u32).unwrap();
    world.resources_mut().insert(10_u32);
    let systems = [
        SystemAccess::of::<QueryMut<'_, &mut u32>>(),
        SystemAccess::of::<Res<'_, u32>>(),
        SystemAccess::of::<()>(),
        SystemAccess::of::<Res<'_, u32>>(),
    ];
    let graph = ConflictGraph::new(&systems);
    let [batch] = graph.batches().collect::<Vec<_>>().try_into().unwrap();
    let mut scopes = world
        .scope_batch(batch.iter().map(|&id| &systems[id]))
        .unwrap();
    let [writer, first, empty, second] = scopes.as_mut_slice() else {
        panic!("expected four scopes in registration order")
    };
    let first_resource = first.resources.get::<u32>().unwrap().unwrap();
    let second_resource = second.resources.get::<u32>().unwrap().unwrap();
    assert!(std::ptr::eq(&*first_resource, &*second_resource));
    *writer.components.get_mut::<u32>(entity).unwrap().unwrap() += *first_resource;
    assert!(empty.components.get::<u32>(entity).is_err());
    drop(scopes);
    assert_eq!(world.get::<u32>(entity), Some(&17));
    assert!(world.scope_batch([]).unwrap().is_empty());
}
