use super::*;
use glam::Vec3;

const UNIT: Aabb = Aabb {
    min: Vec3::ZERO,
    max: Vec3::ONE,
};

fn offset_box(x: f32) -> Aabb {
    let min = Vec3::X * x;
    Aabb {
        min,
        max: min + Vec3::ONE,
    }
}

#[test]
fn inserts_into_empty_and_populated_trees() {
    let mut tree = Bvh::default();
    let first = tree.insert(UNIT, 7).unwrap();
    assert_eq!(tree.leaf_index(first), tree.root_index());

    let far = offset_box(10.0);
    let second = tree.insert(far, 7).unwrap();
    assert_ne!(second, first);

    let third = tree.insert(offset_box(-5.0), 3).unwrap();
    assert_eq!(tree.nodes().len(), 5);
    let root = tree.nodes()[tree.root_index().unwrap()];
    assert_eq!(root.bounds(), offset_box(-5.0).merge(far));
    assert_eq!(tree.leaf_id(tree.leaf_index(first).unwrap()), Some(first));
    assert_eq!(tree.leaf_id(tree.leaf_index(second).unwrap()), Some(second));
    assert_eq!(tree.leaf_id(tree.leaf_index(third).unwrap()), Some(third));
    assert_eq!(tree.query_aabb(&far), vec![7]);
}

#[test]
fn rejects_invalid_insertions_without_changing_the_tree() {
    let mut tree = Bvh::build([(UNIT, 4)]);
    let before = tree.clone();

    assert_eq!(
        tree.insert(
            Aabb {
                min: Vec3::NAN,
                ..UNIT
            },
            6
        ),
        None
    );
    assert_eq!(
        tree.insert(
            Aabb {
                min: Vec3::splat(2.0),
                ..UNIT
            },
            6
        ),
        None
    );
    assert_eq!(tree, before);
}

#[test]
fn removes_only_the_requested_duplicate_and_repairs_indices() {
    let far = offset_box(20.0);
    let mut tree = Bvh::build([(UNIT, 7), (far, 7), (offset_box(3.0), 2)]);
    let id_for = |bounds| {
        let index = tree.nodes().iter().position(|node| {
            matches!(node, BvhNode::Leaf { bounds: leaf_bounds, .. } if *leaf_bounds == bounds)
        }).unwrap();
        tree.leaf_id(index).unwrap()
    };
    let first = id_for(UNIT);
    let survivor = id_for(far);
    let middle = id_for(offset_box(3.0));
    assert_eq!(
        tree.nodes()[tree.leaf_index(survivor).unwrap()].bounds(),
        far
    );

    assert_eq!(tree.remove(first), Some((UNIT, 7)));
    assert_eq!(tree.remove(first), None);
    assert_eq!(tree.leaf_index(first), None);
    assert!(tree.query_aabb(&UNIT).is_empty());
    assert_eq!(tree.query_aabb(&far), vec![7]);
    assert_eq!(tree.remove(middle), Some((offset_box(3.0), 2)));
    assert_eq!(tree.leaf_index(survivor), tree.root_index());
    assert_eq!(tree.remove(survivor), Some((far, 7)));
    assert!(tree.nodes().is_empty());
    assert_eq!(tree.root_index(), None);

    let new_id = tree.insert(UNIT, 7).unwrap();
    assert_ne!(new_id, first);
    assert_ne!(new_id, survivor);
    assert_eq!(tree.remove(first), None);
}
