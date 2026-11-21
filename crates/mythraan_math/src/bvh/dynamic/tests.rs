use super::*;
use glam::{Mat4, Vec3};

use crate::frustum::Frustum;
use crate::ray::Ray;

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

#[test]
fn refits_small_moves_and_reinserts_large_moves_with_the_same_id() {
    let far = offset_box(20.0);
    let mut tree = Bvh::build([(UNIT, 7), (far, 9)]);
    let id = tree.leaf_id(0).unwrap();
    let old_parent = tree.links[0].parent;
    let small_move = offset_box(2.0);

    assert!(tree.update(id, small_move));
    assert_eq!(tree.leaf_index(id), Some(0));
    assert_eq!(tree.links[0].parent, old_parent);
    assert_eq!(
        tree.nodes()[tree.root_index().unwrap()].bounds(),
        small_move.merge(far)
    );
    assert!(tree.query_aabb(&UNIT).is_empty());

    let large_move = offset_box(200.0);
    assert!(tree.update(id, large_move));
    assert!(tree.query_aabb(&small_move).is_empty());
    assert_eq!(tree.query_aabb(&large_move), vec![7]);
    assert_eq!(
        tree.nodes()[tree.root_index().unwrap()].bounds(),
        far.merge(large_move)
    );

    let before = tree.clone();
    assert!(!tree.update(
        id,
        Aabb {
            max: Vec3::NAN,
            ..UNIT
        }
    ));
    assert_eq!(tree, before);
    assert_eq!(tree.remove(id), Some((large_move, 7)));
    assert!(!tree.update(id, UNIT));
}

struct Samples(u64);

impl Samples {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        (self.0 >> 32) as u32
    }

    fn vector(&mut self, min: f32, max: f32) -> Vec3 {
        let mut sample = || min + (max - min) * ((self.next() >> 8) as f32 / 16_777_216.0);
        Vec3::new(sample(), sample(), sample())
    }

    fn bounds(&mut self) -> Aabb {
        let min = self.vector(-20.0, 20.0);
        let max = min + self.vector(0.0, 6.0);
        Aabb { min, max }
    }
}

#[test]
fn keeps_all_queries_equal_to_brute_force_after_seeded_edits() {
    let items = [(UNIT, 3), (offset_box(3.0), 3), (offset_box(9.0), 4)];
    let mut tree = Bvh::build(items);
    let mut model: Vec<_> = tree
        .nodes()
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match *node {
            BvhNode::Leaf { bounds, payload } => {
                Some((tree.leaf_id(index).unwrap(), bounds, payload))
            }
            BvhNode::Branch { .. } => None,
        })
        .collect();
    let mut samples = Samples(0x453a_22b8_1457_c4d2);
    let mut operation_counts = [0; 3];

    for step in 0..1000 {
        let operation = if model.is_empty() {
            0
        } else {
            samples.next() % 3
        };
        match operation {
            0 => {
                let bounds = samples.bounds();
                let payload = samples.next() % 7;
                let id = tree.insert(bounds, payload).unwrap();
                model.push((id, bounds, payload));
            }
            1 => {
                let entry = samples.next() as usize % model.len();
                let (id, bounds, payload) = model.swap_remove(entry);
                assert_eq!(tree.remove(id), Some((bounds, payload)));
                assert_eq!(tree.leaf_index(id), None);
            }
            _ => {
                let entry = samples.next() as usize % model.len();
                let bounds = samples.bounds();
                assert!(tree.update(model[entry].0, bounds));
                model[entry].1 = bounds;
            }
        }
        operation_counts[operation as usize] += 1;
        assert_topology(&tree, &model);

        let origin = samples.vector(-20.0, 20.0);
        let ray = Ray {
            origin,
            direction: model
                .first()
                .map_or(Vec3::X, |&(_, bounds, _)| bounds.center() - origin),
        };
        let expected_hit = model
            .iter()
            .filter_map(|(_, bounds, id)| ray.intersects_aabb(bounds).map(|t| (*id, t)))
            .min_by(|(id_a, t_a), (id_b, t_b)| t_a.total_cmp(t_b).then_with(|| id_a.cmp(id_b)));
        assert_eq!(tree.raycast(&ray), expected_hit, "ray case {step}");

        let query = samples.bounds();
        let mut expected: Vec<_> = model
            .iter()
            .filter_map(|(_, bounds, id)| {
                (bounds.min.x <= query.max.x
                    && query.min.x <= bounds.max.x
                    && bounds.min.y <= query.max.y
                    && query.min.y <= bounds.max.y
                    && bounds.min.z <= query.max.z
                    && query.min.z <= bounds.max.z)
                    .then_some(*id)
            })
            .collect();
        let mut actual = tree.query_aabb(&query);
        expected.sort_unstable();
        actual.sort_unstable();
        assert_eq!(actual, expected, "AABB case {step}");

        let view = Mat4::look_to_rh(samples.vector(-10.0, 10.0), Vec3::NEG_Z, Vec3::Y);
        let projection = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 1.0, 40.0);
        let frustum = Frustum::from_view_projection(&(projection * view)).unwrap();
        let mut expected: Vec<_> = model
            .iter()
            .filter_map(|(_, bounds, id)| frustum.intersects_aabb(bounds).then_some(*id))
            .collect();
        let mut actual = tree.query_frustum(&frustum);
        expected.sort_unstable();
        actual.sort_unstable();
        assert_eq!(actual, expected, "frustum case {step}");
    }

    assert!(operation_counts.into_iter().all(|count| count > 200));
}

fn assert_topology(tree: &Bvh, model: &[(BvhLeafId, Aabb, u32)]) {
    let mut pending: Vec<_> = tree
        .root_index()
        .map(|index| (index, None))
        .into_iter()
        .collect();
    let mut seen = vec![false; tree.nodes().len()];
    let mut leaf_count = 0;
    while let Some((index, parent)) = pending.pop() {
        assert!(!std::mem::replace(&mut seen[index], true));
        assert_eq!(tree.links[index].parent, parent);
        match tree.nodes()[index] {
            BvhNode::Leaf { .. } => {
                assert_eq!(tree.links[index].height, 0);
                leaf_count += 1;
            }
            BvhNode::Branch {
                left,
                right,
                bounds,
            } => {
                assert_eq!(
                    tree.links[index].height,
                    tree.height(left).max(tree.height(right)) + 1
                );
                assert_eq!(
                    bounds,
                    tree.nodes()[left]
                        .bounds()
                        .merge(tree.nodes()[right].bounds())
                );
                pending.extend([(left, Some(index)), (right, Some(index))]);
            }
        }
    }
    assert!(seen.into_iter().all(|node| node));
    assert_eq!(leaf_count, model.len());
    assert_eq!(tree.leaf_indices.len(), model.len());
    for &(id, bounds, payload) in model {
        let index = tree.leaf_index(id).unwrap();
        assert_eq!(tree.leaf_id(index), Some(id));
        assert_eq!(tree.nodes()[index], BvhNode::Leaf { bounds, payload });
    }
}
