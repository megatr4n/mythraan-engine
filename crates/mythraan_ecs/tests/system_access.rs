//! Access declarations through the public ECS API.

use std::any::TypeId;
use std::collections::BTreeSet;

use mythraan_ecs::SystemAccess;
use mythraan_ecs::prelude::Query;

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
