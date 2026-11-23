//! Resource behavior through the public ECS API.

use mythraan_ecs::prelude::{Resources, World};

#[test]
fn stores_one_value_per_type_and_returns_the_replaced_value() {
    let mut resources = Resources::new();
    assert_eq!(resources.insert(String::from("first")), None);
    assert_eq!(resources.insert(40_u32), None);
    assert_eq!(
        resources.insert(String::from("second")).as_deref(),
        Some("first")
    );
    assert_eq!(
        resources.get::<String>().map(String::as_str),
        Some("second")
    );
    assert_eq!(resources.get::<u32>(), Some(&40));
}

#[test]
fn reading_a_missing_resource_returns_none() {
    let mut resources = Resources::new();
    assert!(resources.get::<String>().is_none());
    resources.insert(40_u32);
    assert!(resources.get::<String>().is_none());
}

#[test]
fn mutably_borrows_only_the_requested_resource() {
    let mut resources = Resources::new();
    assert!(resources.get_mut::<String>().is_none());
    resources.insert(String::from("value"));
    resources.insert(40_u32);

    resources.get_mut::<String>().unwrap().push('!');
    assert_eq!(
        resources.get::<String>().map(String::as_str),
        Some("value!")
    );
    assert_eq!(resources.get::<u32>(), Some(&40));
    assert!(resources.get_mut::<bool>().is_none());
}

#[test]
fn removing_a_resource_returns_its_value_and_leaves_other_types() {
    let mut resources = Resources::new();
    assert_eq!(resources.remove::<String>(), None);
    resources.insert(String::from("owned"));
    resources.insert(40_u32);

    assert_eq!(resources.remove::<String>().as_deref(), Some("owned"));
    assert!(resources.get::<String>().is_none());
    assert!(resources.get_mut::<String>().is_none());
    assert_eq!(resources.remove::<String>(), None);
    assert_eq!(resources.get::<u32>(), Some(&40));
}

#[test]
fn different_worlds_have_separate_resources_of_the_same_type() {
    let mut first = World::new();
    let mut second = World::new();

    first.resources_mut().insert(10_u32);
    assert!(second.resources().get::<u32>().is_none());
    second.resources_mut().insert(20_u32);
    *first.resources_mut().get_mut::<u32>().unwrap() += 1;

    assert_eq!(first.resources_mut().remove::<u32>(), Some(11));
    assert!(first.resources().get::<u32>().is_none());
    assert_eq!(second.resources().get::<u32>(), Some(&20));
}

#[test]
fn despawning_an_entity_preserves_a_resource_of_the_same_type() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, 10_u32).unwrap();
    world.resources_mut().insert(20_u32);

    assert!(world.despawn(entity));
    assert_eq!(world.resources().get::<u32>(), Some(&20));
}
