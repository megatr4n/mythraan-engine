//! Oriented bounding boxes.

use glam::{DQuat, Quat, Vec3};

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
        let local = self.rotation.conjugate() * (point.as_dvec3() - self.center.as_dvec3());
        local.abs().cmple(self.half_extents.as_dvec3()).all()
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
}
