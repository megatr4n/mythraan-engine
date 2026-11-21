use super::{Bvh, BvhNode};
use crate::aabb::Aabb;
use crate::frustum::Frustum;
use crate::ray::Ray;

impl Bvh {
    /// Returns the payload and first hit parameter of the nearest leaf box.
    ///
    /// A ray starting in a box hits it at zero. Equal parameters favor the
    /// smaller payload ID.
    #[must_use]
    pub fn raycast(&self, ray: &Ray) -> Option<(u32, f32)> {
        let (origin, direction) = ray.finite_components()?;
        let entry = |index| {
            let bounds = self.nodes().get(index).map(BvhNode::bounds)?;
            Ray::intersect_slabs(
                origin,
                direction,
                bounds.min.as_dvec3(),
                bounds.max.as_dvec3(),
            )
            .map(|parameter| (index, parameter))
        };

        let mut closest = None;
        let mut pending = vec![entry(self.root_index()?)?];
        while let Some((index, parameter)) = pending.pop() {
            if closest.is_some_and(|(_, best)| parameter > best) {
                continue;
            }

            match *self.nodes().get(index)? {
                BvhNode::Leaf { payload, .. } => {
                    if closest.is_none_or(|(best_id, best)| {
                        parameter < best || (parameter == best && payload < best_id)
                    }) {
                        closest = Some((payload, parameter));
                    }
                }
                BvhNode::Branch { left, right, .. } => match (entry(left), entry(right)) {
                    (Some(left), Some(right)) if left.1 <= right.1 => {
                        pending.extend([right, left]);
                    }
                    (Some(left), Some(right)) => pending.extend([left, right]),
                    (Some(hit), None) | (None, Some(hit)) => pending.push(hit),
                    (None, None) => {}
                },
            }
        }

        closest
    }

    /// Returns one payload per leaf box overlapping or touching the query box.
    ///
    /// Payload IDs may repeat. Result order is unspecified.
    #[must_use]
    pub fn query_aabb(&self, query: &Aabb) -> Vec<u32> {
        self.query_bounds(|bounds| {
            bounds.min.cmple(query.max).all() && query.min.cmple(bounds.max).all()
        })
    }

    /// Returns one payload per leaf box that passes the frustum's plane tests.
    ///
    /// The result can include disjoint boxes near frustum edges. Payload IDs
    /// may repeat, and result order is unspecified.
    #[must_use]
    pub fn query_frustum(&self, frustum: &Frustum) -> Vec<u32> {
        self.query_bounds(|bounds| frustum.intersects_aabb(bounds))
    }

    fn query_bounds(&self, overlaps: impl Fn(&Aabb) -> bool) -> Vec<u32> {
        let mut payloads = Vec::new();
        let Some(root) = self.root_index() else {
            return payloads;
        };
        let mut pending = vec![root];

        while let Some(index) = pending.pop() {
            let Some(node) = self.nodes().get(index) else {
                continue;
            };
            if !overlaps(&node.bounds()) {
                continue;
            }

            match *node {
                BvhNode::Leaf { payload, .. } => payloads.push(payload),
                BvhNode::Branch { left, right, .. } => pending.extend([right, left]),
            }
        }

        payloads
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    #[test]
    fn raycast_returns_nearest_payload_and_breaks_ties_by_id() {
        let near = Aabb {
            min: Vec3::new(4.0, 0.0, 0.0),
            max: Vec3::new(5.0, 1.0, 1.0),
        };
        let far = Aabb {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };
        let tree = Bvh::build([(near, 9), (far, 1), (near, 7)]);
        let ray = Ray {
            origin: Vec3::new(10.0, 0.5, 0.5),
            direction: -2.0 * Vec3::X,
        };

        assert_eq!(tree.raycast(&ray), Some((7, 2.5)));
        assert_eq!(
            tree.raycast(&Ray {
                origin: near.center(),
                ..ray
            }),
            Some((7, 0.0))
        );
        assert_eq!(
            tree.raycast(&Ray {
                direction: Vec3::X,
                ..ray
            }),
            None
        );
        assert_eq!(
            tree.raycast(&Ray {
                direction: Vec3::ZERO,
                ..ray
            }),
            None
        );
        assert_eq!(Bvh::default().raycast(&ray), None);
    }

    #[test]
    fn query_aabb_includes_face_contact_and_preserves_duplicate_payloads() {
        let bounds = Aabb {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };
        let inside = Aabb {
            min: Vec3::new(1.0, 0.5, 0.5),
            max: Vec3::new(1.0, 0.5, 0.5),
        };
        let outside = Aabb {
            min: Vec3::new(1.25, 0.0, 0.0),
            max: Vec3::new(2.0, 1.0, 1.0),
        };
        let tree = Bvh::build([(outside, 1), (bounds, 7), (inside, 7)]);

        assert_eq!(tree.query_aabb(&bounds), vec![7, 7]);
        assert!(Bvh::default().query_aabb(&bounds).is_empty());
    }

    #[test]
    fn query_frustum_keeps_visible_and_boundary_boxes() {
        let frustum = Frustum::from_view_projection(&Mat4::IDENTITY).unwrap();
        let inside = Aabb {
            min: Vec3::splat(0.25),
            max: Vec3::splat(0.5),
        };
        let touching = Aabb {
            min: Vec3::Z,
            max: Vec3::Z * 2.0,
        };
        let outside = Aabb {
            min: -Vec3::ONE,
            max: -Vec3::splat(0.25),
        };
        let tree = Bvh::build([(outside, 4), (inside, 2), (touching, 5)]);
        let mut hits = tree.query_frustum(&frustum);
        hits.sort_unstable();

        assert_eq!(hits, vec![2, 5]);
        assert!(Bvh::default().query_frustum(&frustum).is_empty());
    }
}
