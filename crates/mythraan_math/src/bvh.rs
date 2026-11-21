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
}
