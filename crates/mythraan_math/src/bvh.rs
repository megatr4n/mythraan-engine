//! Static bounding volume hierarchies.

use crate::aabb::Aabb;

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
    /// Builds a balanced tree from finite, ordered boxes and their payload IDs.
    #[must_use]
    pub fn build(items: impl IntoIterator<Item = (Aabb, u32)>) -> Self {
        let items: Vec<_> = items.into_iter().collect();
        let mut nodes = Vec::with_capacity(items.len().saturating_mul(2).saturating_sub(1));
        let root = append_subtree(&items, &mut nodes).map(|(index, _)| index);
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

fn append_subtree(items: &[(Aabb, u32)], nodes: &mut Vec<BvhNode>) -> Option<(usize, Aabb)> {
    let node = match items {
        [] => return None,
        &[(bounds, payload)] => BvhNode::Leaf { bounds, payload },
        _ => {
            let middle = items.len() / 2;
            let (left_items, right_items) = items.split_at_checked(middle)?;
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
    use glam::Vec3;

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
}
