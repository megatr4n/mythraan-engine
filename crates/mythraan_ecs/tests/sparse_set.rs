//! Sparse set behavior through the public ECS API.

use mythraan_ecs::SparseSetError;
use mythraan_ecs::prelude::{EntityAllocator, SparseSet};

#[test]
fn inserts_replaces_mutates_and_removes_components() {
    let mut entities = EntityAllocator::new();
    let entity = entities.spawn().unwrap();
    let mut components = SparseSet::new();

    assert!(components.is_empty());
    assert_eq!(components.get(entity), None);
    assert_eq!(components.remove(entity), None);

    assert_eq!(
        components.insert(entity, String::from("first")).unwrap(),
        None
    );
    let old = components.insert(entity, String::from("second")).unwrap();
    assert_eq!(old.as_deref(), Some("first"));
    assert_eq!(components.len(), 1);
    components.get_mut(entity).unwrap().push('!');
    assert_eq!(components.get(entity).map(String::as_str), Some("second!"));
    assert_eq!(components.remove(entity).as_deref(), Some("second!"));
    assert!(components.is_empty());
}

#[test]
fn repairs_the_moved_entity_index_after_swap_remove() {
    let mut entities = EntityAllocator::new();
    let ids: Vec<_> = (0..6).map(|_| entities.spawn().unwrap()).collect();
    let mut components = SparseSet::new();
    for (entity, value) in [(ids[0], 0), (ids[3], 30), (ids[5], 50)] {
        components.insert(entity, value).unwrap();
    }
    assert!(components.get(ids[1]).is_none());
    assert_eq!(components.remove(ids[3]), Some(30));
    assert!(components.get(ids[3]).is_none());
    assert_eq!(components.get(ids[5]), Some(&50));

    for (entity, value) in components.iter_mut() {
        *value += entity.index();
    }
    let pairs: Vec<_> = components.iter().map(|(id, value)| (id, *value)).collect();
    assert_eq!(pairs, [(ids[0], 0), (ids[5], 55)]);
    assert_eq!(components.values(), &[0, 55]);
    assert_eq!(components.remove(ids[5]), Some(55));
    assert_eq!(components.get(ids[0]), Some(&0));
    assert_eq!(components.remove(ids[0]), Some(0));
    assert_eq!(components.iter().len(), 0);
}

#[test]
fn keeps_records_separate_when_the_allocator_reuses_an_index() {
    let mut entities = EntityAllocator::new();
    let mut components = SparseSet::new();
    let original = entities.spawn().unwrap();
    components.insert(original, "original").unwrap();
    assert!(entities.despawn(original));
    let replacement = entities.spawn().unwrap();
    assert_eq!(original.index(), replacement.index());

    assert!(components.get(replacement).is_none());
    assert!(matches!(
        components.insert(replacement, "new"),
        Err(SparseSetError::GenerationMismatch { .. })
    ));
    assert_eq!(components.remove(original), Some("original"));
    assert_eq!(components.insert(replacement, "new").unwrap(), None);

    assert!(components.get(original).is_none());
    assert!(components.get_mut(original).is_none());
    assert!(components.remove(original).is_none());
    assert!(matches!(
        components.insert(original, "stale"),
        Err(SparseSetError::GenerationMismatch { .. })
    ));
    assert_eq!(components.get(replacement), Some(&"new"));
}
