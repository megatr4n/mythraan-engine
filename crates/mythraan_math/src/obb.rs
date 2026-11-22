//! Oriented bounding boxes.

use glam::{DMat3, DQuat, Quat, Vec3};

use crate::ray::Ray;

/// A box with three local half-extents and a rotation into world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obb {
    center: Vec3,
    half_extents: Vec3,
    rotation: DQuat,
}

impl Obb {
    /// Constructs a box from a finite center, nonnegative finite half-extents,
    /// and a finite, nonzero quaternion. The quaternion is normalized.
    #[must_use]
    pub fn new(center: Vec3, half_extents: Vec3, rotation: Quat) -> Option<Self> {
        let rotation = rotation.as_dquat();
        if !center.is_finite()
            || !half_extents.is_finite()
            || !half_extents.cmpge(Vec3::ZERO).all()
            || !rotation.is_finite()
            || rotation.length_squared() == 0.0
        {
            return None;
        }

        Some(Self {
            center,
            half_extents,
            rotation: rotation.normalize(),
        })
    }

    /// Returns the world-space center.
    #[must_use]
    pub fn center(self) -> Vec3 {
        self.center
    }

    /// Returns the local half-extents.
    #[must_use]
    pub fn half_extents(self) -> Vec3 {
        self.half_extents
    }

    /// Returns the rotation from local axes to world axes.
    #[must_use]
    pub fn rotation(self) -> Quat {
        self.rotation.as_quat()
    }

    /// Checks whether a world-space point lies inside or on the box.
    #[must_use]
    pub fn contains_point(self, point: Vec3) -> bool {
        let inverse = DMat3::from_quat(self.rotation.conjugate());
        let local = inverse * (point.as_dvec3() - self.center.as_dvec3());
        local.abs().cmple(self.half_extents.as_dvec3()).all()
    }
}

impl Ray {
    /// Returns the first forward hit parameter for an oriented box.
    ///
    /// Returns zero when the origin is inside or on the box.
    #[must_use]
    pub fn intersects_obb(self, bounds: &Obb) -> Option<f32> {
        let (origin, direction) = self.finite_components()?;
        // Matrix rotation preserves the local ray components in optimized builds.
        let inverse = DMat3::from_quat(bounds.rotation.conjugate());
        let origin = inverse * (origin - bounds.center.as_dvec3());
        let direction = inverse * direction;
        let half_extents = bounds.half_extents.as_dvec3();

        Self::intersect_slabs(origin, direction, -half_extents, half_extents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_local_extents_and_translates_the_center() {
        let center = Vec3::new(10.0, 2.0, 3.0);
        let extents = Vec3::new(2.0, 0.5, 0.5);
        let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let bounds = Obb::new(center, extents, rotation * 2.0).unwrap();

        assert_eq!(bounds.center(), center);
        assert_eq!(bounds.half_extents(), extents);
        assert!(bounds.rotation().abs_diff_eq(rotation, 1.0e-6));
        assert!(bounds.contains_point(center + 1.5 * Vec3::Y));
        assert!(!bounds.contains_point(center + Vec3::X));
    }

    #[test]
    fn accepts_flat_boxes_and_rejects_invalid_construction() {
        let flat = Obb::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0), Quat::IDENTITY).unwrap();

        assert!(flat.contains_point(Vec3::X));
        assert!(!flat.contains_point(Vec3::Z));
        assert_eq!(Obb::new(Vec3::ZERO, -Vec3::X, Quat::IDENTITY), None);
        assert_eq!(Obb::new(Vec3::NAN, Vec3::ONE, Quat::IDENTITY), None);
        assert_eq!(
            Obb::new(Vec3::ZERO, Vec3::ONE, Quat::from_xyzw(0.0, 0.0, 0.0, 0.0)),
            None
        );
    }

    #[test]
    fn ray_hits_rotated_box_without_rescaling_its_parameter() {
        // This unit quaternion maps local X to world Y exactly.
        let bounds = Obb::new(
            Vec3::new(10.0, 2.0, 3.0),
            Vec3::new(2.0, 0.5, 0.5),
            Quat::from_xyzw(0.5, 0.5, 0.5, 0.5),
        )
        .unwrap();
        let ray = Ray {
            origin: bounds.center() - 6.0 * Vec3::Y,
            direction: 2.0 * Vec3::Y,
        };
        let outside_parallel = Ray {
            origin: ray.origin + Vec3::X,
            ..ray
        };

        assert_eq!(ray.intersects_obb(&bounds), Some(2.0));
        assert_eq!(outside_parallel.intersects_obb(&bounds), None);
        assert!(bounds.contains_point(bounds.center() + 1.5 * Vec3::Y));
        assert!(!bounds.contains_point(bounds.center() + Vec3::X));
        assert_eq!(
            Ray {
                origin: bounds.center(),
                ..ray
            }
            .intersects_obb(&bounds),
            Some(0.0)
        );
    }
}
