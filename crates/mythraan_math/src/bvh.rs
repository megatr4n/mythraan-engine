//! Bounding volume hierarchies.

use std::collections::BTreeMap;

use glam::Vec3;

use crate::aabb::Aabb;

mod dynamic;
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

/// The identity of a leaf within one BVH, independent of its node index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BvhLeafId(usize);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct NodeLinks {
    leaf: Option<BvhLeafId>,
    parent: Option<usize>,
    height: usize,
}

/// A binary hierarchy over axis-aligned bounding boxes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bvh {
    nodes: Vec<BvhNode>,
    root: Option<usize>,
    links: Vec<NodeLinks>,
    leaf_indices: BTreeMap<BvhLeafId, usize>,
    next_leaf_id: usize,
}

impl Bvh {
    /// Builds a balanced tree by splitting finite, ordered boxes at the centroid median.
    ///
    /// Payload IDs are preserved and may repeat. Equal split keys follow input order.
    #[must_use]
    pub fn build(items: impl IntoIterator<Item = (Aabb, u32)>) -> Self {
        let inputs: Vec<_> = items.into_iter().collect();
        let mut items: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(input_index, (bounds, _))| BuildItem {
                centroid: bounds.center(),
                input_index,
            })
            .collect();
        let mut nodes = Vec::with_capacity(items.len().saturating_mul(2).saturating_sub(1));
        let root = append_subtree(&mut items, &inputs, &mut nodes).map(|(index, _)| index);
        let mut links = vec![NodeLinks::default(); nodes.len()];
        let mut next_leaf_id = 0;
        for (index, node) in nodes.iter().enumerate() {
            match *node {
                BvhNode::Leaf { .. } => {
                    if let Some(link) = links.get_mut(index) {
                        link.leaf = Some(BvhLeafId(next_leaf_id));
                    }
                    next_leaf_id += 1;
                }
                BvhNode::Branch { left, right, .. } => {
                    let mut height = 0;
                    for child in [left, right] {
                        if let Some(link) = links.get_mut(child) {
                            link.parent = Some(index);
                            height = height.max(link.height + 1);
                        }
                    }
                    if let Some(link) = links.get_mut(index) {
                        link.height = height;
                    }
                }
            }
        }
        let leaf_indices = links
            .iter()
            .enumerate()
            .filter_map(|(index, link)| Some((link.leaf?, index)))
            .collect();
        Self {
            nodes,
            root,
            links,
            next_leaf_id,
            leaf_indices,
        }
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

    /// Returns the stable ID of the leaf at this node index.
    #[must_use]
    pub fn leaf_id(&self, node_index: usize) -> Option<BvhLeafId> {
        self.links.get(node_index)?.leaf
    }

    /// Returns the leaf's current node index, or `None` if it is absent.
    #[must_use]
    pub fn leaf_index(&self, id: BvhLeafId) -> Option<usize> {
        self.leaf_indices.get(&id).copied()
    }
}

struct BuildItem {
    centroid: Vec3,
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

    fn partition(self, items: &mut [BuildItem], middle: usize) {
        match self {
            Self::X => partition::<0>(items, middle),
            Self::Y => partition::<1>(items, middle),
            Self::Z => partition::<2>(items, middle),
        }
    }
}

fn partition<const AXIS: usize>(items: &mut [BuildItem], middle: usize) {
    let compare = |left: &BuildItem, right: &BuildItem| {
        left.centroid[AXIS]
            .total_cmp(&right.centroid[AXIS])
            .then_with(|| left.input_index.cmp(&right.input_index))
    };
    if let [left, right] = items {
        if compare(left, right).is_gt() {
            std::mem::swap(left, right);
        }
    } else {
        items.select_nth_unstable_by(middle, compare);
    }
}

fn append_subtree(
    items: &mut [BuildItem],
    inputs: &[(Aabb, u32)],
    nodes: &mut Vec<BvhNode>,
) -> Option<(usize, Aabb)> {
    let node = match items {
        [] => return None,
        [item] => {
            let &(bounds, payload) = inputs.get(item.input_index)?;
            BvhNode::Leaf { bounds, payload }
        }
        _ => {
            let middle = items.len() / 2;
            Axis::widest(items).partition(items, middle);
            let (left_items, right_items) = items.split_at_mut_checked(middle)?;
            let (left, left_bounds) = append_subtree(left_items, inputs, nodes)?;
            let (right, right_bounds) = append_subtree(right_items, inputs, nodes)?;
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
mod tests;
