//! Resource behavior through the public ECS API.

use mythraan_ecs::prelude::Resources;

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
