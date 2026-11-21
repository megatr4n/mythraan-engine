//! Static bounding volume hierarchies.

use glam::Vec3;

use crate::aabb::Aabb;

mod queries;

/// A node in a bounding volume hierarchy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BvhNode {
    /// A leaf holding one input item.
    Leaf {
        /// Bounds of the item.
        bounds: Aabb,
        /// Caller-provided item identifier.
        payload: u32,
    },
    /// An internal node enclosing two child nodes.
    Branch {
        /// Bounds enclosing both children.
        bounds: Aabb,
        /// Left child index in the tree's node slice.
        left: usize,
        /// Right child index in the tree's node slice.
        right: usize,
    },
}

impl BvhNode {
    /// Returns the bounds of this node.
    #[must_use]
    pub fn bounds(&self) -> Aabb {
        match self {
            Self::Leaf { bounds, .. } | Self::Branch { bounds, .. } => *bounds,
        }
    }
}

/// A static binary hierarchy over axis-aligned bounding boxes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bvh {
    nodes: Vec<BvhNode>,
    root: Option<usize>,
}

impl Bvh {
    /// Builds a balanced tree by splitting finite, ordered boxes at the centroid median.
    ///
    /// Payload IDs are preserved and may repeat. Equal split keys follow input order.
    #[must_use]
    pub fn build(items: impl IntoIterator<Item = (Aabb, u32)>) -> Self {
        let mut items: Vec<_> = items
            .into_iter()
            .enumerate()
            .map(|(input_index, (bounds, payload))| BuildItem {
                bounds,
                centroid: bounds.center(),
                payload,
                input_index,
            })
            .collect();
        let mut nodes = Vec::with_capacity(items.len().saturating_mul(2).saturating_sub(1));
        let root = append_subtree(&mut items, &mut nodes).map(|(index, _)| index);
        Self { nodes, root }
    }

    /// Returns the node slice addressed by the root and child indices.
    #[must_use]
    pub fn nodes(&self) -> &[BvhNode] {
        &self.nodes
    }

    /// Returns the root's index, or `None` for an empty tree.
    #[must_use]
    pub fn root_index(&self) -> Option<usize> {
        self.root
    }
}

struct BuildItem {
    bounds: Aabb,
    centroid: Vec3,
    payload: u32,
    input_index: usize,
}

enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn widest(items: &[BuildItem]) -> Self {
        let (min, max) = items.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), item| (min.min(item.centroid), max.max(item.centroid)),
        );
        let span = max.as_dvec3() - min.as_dvec3();
        if span.x >= span.y && span.x >= span.z {
            Self::X
        } else if span.y >= span.z {
            Self::Y
        } else {
            Self::Z
        }
    }

    fn component(&self, centroid: Vec3) -> f32 {
        match self {
            Self::X => centroid.x,
            Self::Y => centroid.y,
            Self::Z => centroid.z,
        }
    }
}

fn append_subtree(items: &mut [BuildItem], nodes: &mut Vec<BvhNode>) -> Option<(usize, Aabb)> {
    let node = match items {
        [] => return None,
        [item] => BvhNode::Leaf {
            bounds: item.bounds,
            payload: item.payload,
        },
        _ => {
            let middle = items.len() / 2;
            let axis = Axis::widest(items);
            items.select_nth_unstable_by(middle, |left, right| {
                axis.component(left.centroid)
                    .total_cmp(&axis.component(right.centroid))
                    .then_with(|| left.input_index.cmp(&right.input_index))
            });
            let (left_items, right_items) = items.split_at_mut_checked(middle)?;
            let (left, left_bounds) = append_subtree(left_items, nodes)?;
            let (right, right_bounds) = append_subtree(right_items, nodes)?;
            BvhNode::Branch {
                bounds: left_bounds.merge(right_bounds),
                left,
                right,
            }
        }
    };

    let result = (nodes.len(), node.bounds());
    nodes.push(node);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_node_kinds_expose_their_bounds() {
        let bounds = Aabb {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };

        assert_eq!(BvhNode::Leaf { bounds, payload: 7 }.bounds(), bounds);
        assert_eq!(
            BvhNode::Branch {
                bounds,
                left: 0,
                right: 1
            }
            .bounds(),
            bounds
        );
    }

    #[test]
    fn builds_empty_and_single_item_trees() {
        let empty = Bvh::build([]);
        assert_eq!(empty.root_index(), None);
        assert!(empty.nodes().is_empty());

        let bounds = Aabb {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };
        let single = Bvh::build([(bounds, u32::MAX)]);
        assert_eq!(single.root_index(), Some(0));
        assert_eq!(
            single.nodes(),
            &[BvhNode::Leaf {
                bounds,
                payload: u32::MAX
            }]
        );
    }

    #[test]
    fn a_branch_encloses_both_children() {
        let left_bounds = Aabb {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };
        let right_bounds = Aabb {
            min: Vec3::splat(3.0),
            max: Vec3::splat(4.0),
        };
        let tree = Bvh::build([(left_bounds, 7), (right_bounds, 2)]);
        let root = tree.root_index().unwrap();
        let BvhNode::Branch {
            bounds,
            left,
            right,
        } = tree.nodes()[root]
        else {
            panic!("expected a branch");
        };

        assert_eq!(bounds, left_bounds.merge(right_bounds));
        assert_eq!(
            tree.nodes()[left],
            BvhNode::Leaf {
                bounds: left_bounds,
                payload: 7
            }
        );
        assert_eq!(
            tree.nodes()[right],
            BvhNode::Leaf {
                bounds: right_bounds,
                payload: 2
            }
        );
    }

    #[test]
    fn splits_at_the_centroid_median_on_the_widest_axis() {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            let items = [12.0, -10.0, 10.0, -12.0].map(|coordinate| {
                let point = axis * coordinate;
                (
                    Aabb {
                        min: point,
                        max: point,
                    },
                    0,
                )
            });
            let tree = Bvh::build(items);
            let BvhNode::Branch { left, right, .. } = tree.nodes()[tree.root_index().unwrap()]
            else {
                panic!("expected a branch");
            };

            assert_eq!(tree.nodes()[left].bounds(), items[1].0.merge(items[3].0));
            assert_eq!(tree.nodes()[right].bounds(), items[0].0.merge(items[2].0));
        }
    }

    #[test]
    fn reaches_every_leaf_and_encloses_its_bounds() {
        for count in [2_u32, 3, 17, 64, 127] {
            let items: Vec<_> = (0..count)
                .map(|id| {
                    let min = Vec3::new(
                        f32::from(((id * 37) % 43) as u8),
                        f32::from(((id * 11) % 31) as u8),
                        f32::from(((id * 23) % 19) as u8),
                    );
                    let max = min + Vec3::new(0.0, 0.5, 1.0);
                    (Aabb { min, max }, id)
                })
                .collect();
            let tree = Bvh::build(items.iter().copied());
            let nodes = tree.nodes();
            let mut seen = vec![false; nodes.len()];
            let mut pending = vec![tree.root_index().unwrap()];
            let mut payloads = Vec::new();

            while let Some(index) = pending.pop() {
                assert!(
                    !std::mem::replace(&mut seen[index], true),
                    "node visited twice"
                );
                match nodes[index] {
                    BvhNode::Leaf { bounds, payload } => {
                        assert_eq!(bounds, items[payload as usize].0);
                        payloads.push(payload);
                    }
                    BvhNode::Branch {
                        bounds,
                        left,
                        right,
                    } => {
                        for child in [left, right] {
                            let child_bounds = nodes[child].bounds();
                            assert!(bounds.contains_point(child_bounds.min));
                            assert!(bounds.contains_point(child_bounds.max));
                            pending.push(child);
                        }
                    }
                }
            }

            assert!(seen.into_iter().all(|reached| reached));
            payloads.sort_unstable();
            assert_eq!(payloads, (0..count).collect::<Vec<_>>());
            assert_eq!(nodes.len(), items.len() * 2 - 1);
        }
    }

    #[test]
    fn keeps_coincident_centroids_balanced_and_preserves_duplicate_ids() {
        let point = Aabb {
            min: Vec3::ONE,
            max: Vec3::ONE,
        };
        let items: Vec<_> = (0..33).rev().map(|index| (point, index % 5)).collect();
        let tree = Bvh::build(items.iter().copied());
        assert_eq!(tree, Bvh::build(items.iter().copied()));

        let mut leaf_counts = Vec::new();
        let mut payloads = Vec::new();
        for node in tree.nodes() {
            let count = match *node {
                BvhNode::Leaf { payload, .. } => {
                    payloads.push(payload);
                    1_usize
                }
                BvhNode::Branch { left, right, .. } => {
                    let left_count: usize = leaf_counts[left];
                    let right_count = leaf_counts[right];
                    assert!(left_count.abs_diff(right_count) <= 1);
                    left_count + right_count
                }
            };
            leaf_counts.push(count);
        }

        assert_eq!(leaf_counts[tree.root_index().unwrap()], items.len());
        assert_eq!(
            payloads,
            items.into_iter().map(|(_, id)| id).collect::<Vec<_>>()
        );
    }
}
