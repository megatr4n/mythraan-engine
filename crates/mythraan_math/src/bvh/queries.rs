use super::{Bvh, BvhNode};
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aabb::Aabb;
    use glam::Vec3;

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
}
