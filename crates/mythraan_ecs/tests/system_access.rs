//! Access declarations through the public ECS API.

use std::any::TypeId;
use std::collections::BTreeSet;

use mythraan_ecs::SystemAccess;
use mythraan_ecs::prelude::{Query, Res, Resources};

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
