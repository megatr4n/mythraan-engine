//! World behavior through the public ECS API.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use mythraan_ecs::WorldInsertError;
use mythraan_ecs::prelude::World;

#[test]
fn spawns_distinct_live_entities() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();

    assert_ne!(first, second);
    assert!(world.is_alive(first));
    assert!(world.is_alive(second));
}

#[test]
fn insert_returns_previous_component_of_the_same_type() {
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    assert_eq!(world.insert(entity, String::from("first")).unwrap(), None);
    assert_eq!(world.insert(entity, 5_u32).unwrap(), None);
    assert_eq!(
        world
            .insert(entity, String::from("second"))
            .unwrap()
            .as_deref(),
        Some("first")
    );
}

#[test]
fn reads_multiple_component_types_without_mixing_entities() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    world.insert(first, String::from("first")).unwrap();
    world.insert(first, 100_u32).unwrap();
    world.insert(second, String::from("second")).unwrap();

    assert_eq!(
        world.get::<String>(first).map(String::as_str),
        Some("first")
    );
    assert_eq!(
        world.get::<String>(second).map(String::as_str),
        Some("second")
    );
    assert_eq!(world.get::<u32>(first), Some(&100));
    assert_eq!(world.get::<u32>(second), None);
    assert_eq!(world.get::<bool>(first), None);
}

#[test]
fn mutates_only_the_component_borrowed() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    world.insert(first, 10_u32).unwrap();
    world.insert(first, false).unwrap();
    world.insert(second, 20_u32).unwrap();

    *world.get_mut::<u32>(first).unwrap() += 1;
    assert_eq!(world.get::<u32>(first), Some(&11));
    assert_eq!(world.get::<u32>(second), Some(&20));
    assert_eq!(world.get::<bool>(first), Some(&false));
    assert!(world.get_mut::<bool>(second).is_none());
}

#[test]
fn removes_one_component_and_preserves_other_records() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    world.insert(first, String::from("first")).unwrap();
    world.insert(first, 11_u32).unwrap();
    world.insert(second, String::from("second")).unwrap();

    assert_eq!(world.remove::<String>(first).as_deref(), Some("first"));
    assert_eq!(world.remove::<String>(first), None);
    assert_eq!(world.get::<u32>(first), Some(&11));
    assert_eq!(
        world.get::<String>(second).map(String::as_str),
        Some("second")
    );
    assert!(world.is_alive(first));
}

#[test]
fn despawn_cleans_every_component_storage_before_slot_reuse() {
    let mut world = World::new();
    let dead = world.spawn().unwrap();
    let survivor = world.spawn().unwrap();
    world.insert(dead, String::from("old")).unwrap();
    world.insert(dead, 5_u32).unwrap();
    world.insert(survivor, 9_u32).unwrap();
    assert!(world.despawn(dead));

    let replacement = world.spawn().unwrap();
    assert_eq!(replacement.index(), dead.index());
    assert_ne!(replacement.generation(), dead.generation());
    assert!(world.get::<String>(replacement).is_none());
    assert!(world.get::<u32>(replacement).is_none());
    assert_eq!(
        world.insert(replacement, String::from("new")).unwrap(),
        None
    );
    assert_eq!(world.insert(replacement, 7_u32).unwrap(), None);
    assert_eq!(world.get::<u32>(survivor), Some(&9));
}

#[test]
fn stale_id_cannot_read_or_modify_its_replacement() {
    let mut world = World::new();
    let old = world.spawn().unwrap();
    assert!(world.despawn(old));
    assert!(!world.despawn(old));
    let live = world.spawn().unwrap();
    assert_eq!(old.index(), live.index());
    world.insert(live, 44_u32).unwrap();

    assert!(!world.is_alive(old));
    assert!(world.get::<u32>(old).is_none());
    assert!(world.get_mut::<u32>(old).is_none());
    assert!(world.remove::<u32>(old).is_none());
    assert!(matches!(
        world.insert(old, 55_u32),
        Err(WorldInsertError::EntityNotAlive { entity }) if entity == old
    ));
    assert!(!world.despawn(old));
    assert!(world.is_alive(live));
    assert_eq!(world.get::<u32>(live), Some(&44));
}

#[test]
fn despawn_drops_owned_components_of_each_type_once() {
    struct CountDrops<const KIND: u8>(Arc<AtomicUsize>);

    impl<const KIND: u8> Drop for CountDrops<KIND> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    let drops = Arc::new(AtomicUsize::new(0));
    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world
        .insert(entity, CountDrops::<0>(Arc::clone(&drops)))
        .unwrap();
    world
        .insert(entity, CountDrops::<1>(Arc::clone(&drops)))
        .unwrap();

    assert!(world.despawn(entity));
    assert_eq!(drops.load(Ordering::Relaxed), 2);
    assert!(!world.despawn(entity));
    drop(world);
    assert_eq!(drops.load(Ordering::Relaxed), 2);
}

#[test]
fn despawn_can_retry_after_a_component_destructor_unwinds() {
    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("test component destructor");
        }
    }

    let mut world = World::new();
    let entity = world.spawn().unwrap();
    world.insert(entity, PanicOnDrop).unwrap();
    world.insert(entity, 42_u32).unwrap();
    let first = catch_unwind(AssertUnwindSafe(|| world.despawn(entity)));
    assert!(first.is_err());
    assert!(world.is_alive(entity));
    assert!(world.get::<PanicOnDrop>(entity).is_none());

    assert!(world.despawn(entity));
    let replacement = world.spawn().unwrap();
    assert_eq!(replacement.index(), entity.index());
    assert_ne!(replacement.generation(), entity.generation());
    assert_eq!(world.insert(replacement, 1_u32).unwrap(), None);
}
