use super::{Bvh, BvhLeafId, BvhNode, NodeLinks};
use crate::aabb::Aabb;

impl Bvh {
    /// Inserts a leaf and returns its ID, which survives node relocation.
    ///
    /// Returns `None` for non-finite or inverted bounds or exhausted IDs.
    #[must_use]
    pub fn insert(&mut self, bounds: Aabb, payload: u32) -> Option<BvhLeafId> {
        if !valid_bounds(bounds) {
            return None;
        }
        let id = BvhLeafId(self.next_leaf_id);
        self.next_leaf_id = self.next_leaf_id.checked_add(1)?;
        let index = self.nodes.len();
        self.nodes.push(BvhNode::Leaf { bounds, payload });
        self.links.push(NodeLinks {
            leaf: Some(id),
            ..NodeLinks::default()
        });
        self.leaf_indices.insert(id, index);
        self.attach_leaf(index, bounds);
        Some(id)
    }

    fn attach_leaf(&mut self, leaf: usize, bounds: Aabb) {
        let Some(root) = self.root else {
            self.root = Some(leaf);
            return;
        };

        let sibling = self.insertion_sibling(root, bounds);
        let Some((sibling_node, link)) = self.nodes.get(sibling).zip(self.links.get(sibling))
        else {
            return;
        };
        let parent = link.parent;
        let branch = self.nodes.len();
        self.nodes.push(BvhNode::Branch {
            bounds: sibling_node.bounds().merge(bounds),
            left: sibling,
            right: leaf,
        });
        self.links.push(NodeLinks {
            parent,
            height: link.height + 1,
            ..NodeLinks::default()
        });
        self.replace_child(parent, sibling, branch);
        self.set_parent(sibling, Some(branch));
        self.set_parent(leaf, Some(branch));
        self.refit_ancestors(parent);
    }

    fn insertion_sibling(&self, mut index: usize, bounds: Aabb) -> usize {
        while let Some(&BvhNode::Branch { left, right, .. }) = self.nodes.get(index) {
            let Some((left_node, right_node)) = self.nodes.get(left).zip(self.nodes.get(right))
            else {
                break;
            };
            let left_height = self.height(left);
            let right_height = self.height(right);
            let left_growth = area_growth(left_node.bounds(), bounds);
            let right_growth = area_growth(right_node.bounds(), bounds);

            index = if left_height + 1 < right_height {
                left
            } else if right_height + 1 < left_height {
                right
            } else if left_growth < right_growth
                || (left_growth == right_growth && left_height <= right_height)
            {
                left
            } else {
                right
            };
        }
        index
    }

    fn height(&self, index: usize) -> usize {
        self.links.get(index).map_or(0, |link| link.height)
    }

    fn set_parent(&mut self, index: usize, parent: Option<usize>) {
        if let Some(link) = self.links.get_mut(index) {
            link.parent = parent;
        }
    }

    fn replace_child(&mut self, parent: Option<usize>, old: usize, replacement: usize) {
        let Some(parent) = parent else {
            self.root = Some(replacement);
            return;
        };
        if let Some(BvhNode::Branch { left, right, .. }) = self.nodes.get_mut(parent) {
            if *left == old {
                *left = replacement;
            } else if *right == old {
                *right = replacement;
            }
        }
    }

    fn refit_ancestors(&mut self, mut index: Option<usize>) {
        while let Some(branch) = index {
            let Some(&BvhNode::Branch { left, right, .. }) = self.nodes.get(branch) else {
                break;
            };
            let Some((left_node, right_node)) = self.nodes.get(left).zip(self.nodes.get(right))
            else {
                break;
            };
            let bounds = left_node.bounds().merge(right_node.bounds());
            let height = self.height(left).max(self.height(right)) + 1;
            if let Some(BvhNode::Branch {
                bounds: current, ..
            }) = self.nodes.get_mut(branch)
            {
                *current = bounds;
            }
            let Some(link) = self.links.get_mut(branch) else {
                break;
            };
            link.height = height;
            index = link.parent;
        }
    }
}

fn valid_bounds(bounds: Aabb) -> bool {
    bounds.min.is_finite() && bounds.max.is_finite() && bounds.min.cmple(bounds.max).all()
}

fn area_growth(current: Aabb, addition: Aabb) -> f64 {
    let area = |bounds: Aabb| {
        let size = bounds.max.as_dvec3() - bounds.min.as_dvec3();
        size.x * size.y + size.y * size.z + size.z * size.x
    };
    area(current.merge(addition)) - area(current)
}

#[cfg(test)]
mod tests;
