//! Access declarations through the public ECS API.

use std::any::TypeId;
use std::collections::BTreeSet;

use mythraan_ecs::SystemAccess;
use mythraan_ecs::prelude::{Query, QueryMut, Res, ResMut, Resources};

#[test]
fn shared_queries_declare_every_component_type_once() {
    let access = SystemAccess::of::<Query<'_, (&u32, &bool, &u32, &i32)>>();
    assert_eq!(
        access.components().reads(),
        &BTreeSet::from([
            TypeId::of::<u32>(),
            TypeId::of::<bool>(),
            TypeId::of::<i32>()
        ])
    );

    let single = SystemAccess::of::<Query<'_, &String>>();
    assert_eq!(
        single.components().reads(),
        &BTreeSet::from([TypeId::of::<String>()])
    );
}

#[test]
fn shared_resources_have_their_own_access_set() {
    let access = SystemAccess::of::<Res<'_, u32>>();
    assert_eq!(
        access.resources().reads(),
        &BTreeSet::from([TypeId::of::<u32>()])
    );
    assert!(access.components().reads().is_empty());

    let mut resources = Resources::new();
    assert!(Res::<u32>::from_resources(&resources).is_none());
    resources.insert(12_u32);
    assert_eq!(*Res::<u32>::from_resources(&resources).unwrap(), 12);
}

#[test]
fn mutable_resources_declare_exclusive_access() {
    let access = SystemAccess::of::<ResMut<'_, u32>>();
    assert_eq!(
        access.resources().writes(),
        &BTreeSet::from([TypeId::of::<u32>()])
    );
    assert!(access.resources().reads().is_empty());
    assert!(access.components().writes().is_empty());

    let mut resources = Resources::new();
    assert!(ResMut::<u32>::from_resources(&mut resources).is_none());
    resources.insert(12_u32);
    *ResMut::<u32>::from_resources(&mut resources).unwrap() += 4;
    assert_eq!(resources.get::<u32>(), Some(&16));
}

#[test]
fn mutable_queries_declare_writes_and_optional_reads() {
    type Mixed<'world> = QueryMut<'world, (&'world u32, Option<&'world String>, &'world mut bool)>;

    let access = SystemAccess::of::<Mixed<'_>>();
    assert_eq!(
        access.components().reads(),
        &BTreeSet::from([TypeId::of::<u32>(), TypeId::of::<String>()])
    );
    assert_eq!(
        access.components().writes(),
        &BTreeSet::from([TypeId::of::<bool>()])
    );
}
