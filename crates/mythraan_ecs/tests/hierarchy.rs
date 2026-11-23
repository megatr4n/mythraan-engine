//! Entity hierarchy behavior through the public ECS API.

use std::panic::{AssertUnwindSafe, catch_unwind};

use mythraan_ecs::HierarchyError;
use mythraan_ecs::prelude::{Parent, World};

#[test]
fn moving_a_child_updates_both_parents_and_detaching_makes_it_a_root() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    let child = world.spawn().unwrap();
    world.set_parent(child, Some(first)).unwrap();
    world.set_parent(child, Some(first)).unwrap();
    assert_eq!(world.children(first).unwrap().as_slice(), [child]);

    world.set_parent(child, Some(second)).unwrap();
    assert!(world.children(first).is_none());
    assert_eq!(world.children(second).unwrap().as_slice(), [child]);
    assert_eq!(world.parent(child).map(Parent::entity), Some(second));

    world.set_parent(child, None).unwrap();
    assert!(world.children(second).is_none());
    assert_eq!(world.parent(child), None);
    assert!(world.is_alive(child));
}

#[test]
fn a_cycle_leaves_the_original_links_intact() {
    let mut world = World::new();
    let root = world.spawn().unwrap();
    let child = world.spawn().unwrap();
    let leaf = world.spawn().unwrap();
    world.set_parent(child, Some(root)).unwrap();
    world.set_parent(leaf, Some(child)).unwrap();

    for parent in [root, leaf] {
        assert!(
            matches!(world.set_parent(root, Some(parent)), Err(HierarchyError::Cycle(entity)) if entity == root)
        );
    }
    assert_eq!(world.parent(root), None);
    assert_eq!(world.children(root).unwrap().as_slice(), [child]);
    assert_eq!(world.children(child).unwrap().as_slice(), [leaf]);
}

#[test]
fn despawning_a_branch_removes_its_descendants_and_unlinks_it() {
    let mut world = World::new();
    let root = world.spawn().unwrap();
    let child = world.spawn().unwrap();
    let sibling = world.spawn().unwrap();
    let leaf = world.spawn().unwrap();
    world.set_parent(child, Some(root)).unwrap();
    world.set_parent(sibling, Some(root)).unwrap();
    world.set_parent(leaf, Some(child)).unwrap();
    world.insert(leaf, 7_u32).unwrap();

    assert!(world.despawn(child));
    assert!(!world.is_alive(child));
    assert!(!world.is_alive(leaf));
    assert_eq!(world.query::<&u32>().count(), 0);
    assert_eq!(world.children(root).unwrap().as_slice(), [sibling]);
    assert_eq!(world.parent(sibling).map(Parent::entity), Some(root));
}

#[test]
fn stale_links_cannot_reparent_or_unlink_live_entities() {
    let mut world = World::new();
    let original = world.spawn().unwrap();
    let root = world.spawn().unwrap();
    let child = world.spawn().unwrap();
    world.set_parent(original, Some(root)).unwrap();
    assert!(world.despawn(original));
    let replacement = world.spawn().unwrap();
    assert_eq!(replacement.index(), original.index());
    assert_ne!(replacement, original);

    world.set_parent(child, Some(root)).unwrap();
    assert!(
        matches!(world.set_parent(child, Some(original)), Err(HierarchyError::EntityNotAlive(entity)) if entity == original)
    );
    assert!(
        matches!(world.set_parent(original, Some(root)), Err(HierarchyError::EntityNotAlive(entity)) if entity == original)
    );
    assert_eq!(world.parent(child).map(Parent::entity), Some(root));
    assert_eq!(world.parent(replacement), None);
    assert_eq!(world.children(root).unwrap().as_slice(), [child]);
}

#[test]
fn a_deep_hierarchy_is_despawned_without_call_stack_recursion() {
    const DEPTH: usize = 16_384;
    let mut world = World::new();
    let mut root = world.spawn().unwrap();
    let mut lineage = Vec::with_capacity(DEPTH);
    lineage.push(root);
    for _ in 1..DEPTH {
        let parent = world.spawn().unwrap();
        world.set_parent(root, Some(parent)).unwrap();
        lineage.push(parent);
        root = parent;
    }

    assert!(world.despawn(root));
    assert!(lineage.into_iter().all(|entity| !world.is_alive(entity)));
    assert!(world.children(root).is_none());
}

#[test]
fn a_panicking_descendant_can_be_cleaned_up_by_retrying_the_root() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("component destructor unwound");
        }
    }

    let mut world = World::new();
    let root = world.spawn().unwrap();
    let failing = world.spawn().unwrap();
    let removed = world.spawn().unwrap();
    world.set_parent(failing, Some(root)).unwrap();
    world.set_parent(removed, Some(failing)).unwrap();
    world.insert(failing, PanicOnDrop).unwrap();

    assert!(catch_unwind(AssertUnwindSafe(|| world.despawn(root))).is_err());
    assert!(!world.is_alive(removed));
    assert!(world.is_alive(root));
    assert!(world.is_alive(failing));
    assert_eq!(world.children(root).unwrap().as_slice(), [failing]);
    assert!(world.despawn(root));
    assert!(!world.is_alive(failing));
}
