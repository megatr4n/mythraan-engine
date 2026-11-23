//! Safe, isolated storage borrows for a system batch.

use std::any::TypeId;

use mythraan_ecs::prelude::{Query, QueryMut, Res, ResMut, SystemAccess, World};
use mythraan_ecs::{ScopeError, StorageKind};

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
