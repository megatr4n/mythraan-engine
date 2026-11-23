//! Access declarations through the public ECS API.

use std::any::TypeId;
use std::collections::BTreeSet;

use mythraan_ecs::SystemAccess;
use mythraan_ecs::prelude::{
    Added, Changed, FilteredQuery, Query, QueryMut, Res, ResMut, Resources, With, Without, World,
};

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

#[test]
fn filtered_queries_declare_reads_of_filter_types() {
    type Filters = (With<bool>, Without<u64>, Added<i32>, Changed<String>);
    type Parameter<'world> = FilteredQuery<'world, (&'world u16, &'world mut String), Filters>;

    let access = SystemAccess::of::<Parameter<'_>>();
    assert_eq!(
        access.components().reads(),
        &BTreeSet::from([
            TypeId::of::<u16>(),
            TypeId::of::<bool>(),
            TypeId::of::<u64>(),
            TypeId::of::<i32>(),
        ])
    );
    assert_eq!(
        access.components().writes(),
        &BTreeSet::from([TypeId::of::<String>()])
    );
}

#[test]
fn typed_filter_parameters_use_the_world_query_rules() {
    let mut world = World::new();
    let matching = world.spawn().unwrap();
    let other = world.spawn().unwrap();
    world.insert(matching, true).unwrap();
    let checkpoint = world.change_tick();
    world.advance_change_tick().unwrap();
    world.insert(matching, 10_u32).unwrap();
    world.insert(other, 20_u32).unwrap();
    world.advance_change_tick().unwrap();

    type Match<'world> = FilteredQuery<'world, &'world mut u32, (With<bool>, Added<u32>)>;
    Match::from_world(&mut world)
        .unwrap()
        .for_each(|_, _| panic!("no current additions"));
    Match::from_world_since(&mut world, checkpoint)
        .unwrap()
        .for_each(|entity, value| {
            assert_eq!(entity, matching);
            *value += 1;
        });
    assert_eq!(world.get::<u32>(matching), Some(&11));
    assert_eq!(world.get::<u32>(other), Some(&20));
}
