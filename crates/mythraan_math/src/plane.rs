//! Oriented planes in three dimensions.

use glam::{DVec3, DVec4, Vec3};

use crate::ray::Ray;

/// A plane with a unit normal, defined by `normal.dot(point) + offset = 0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    normal: DVec3,
    offset: f64,
}

impl Plane {
    /// Builds a plane through a finite point with a finite, nonzero normal.
    #[must_use]
    pub fn from_point_normal(point: Vec3, normal: Vec3) -> Option<Self> {
        if !point.is_finite() {
            return None;
        }

        let mut plane = Self::from_equation(normal.as_dvec3().extend(0.0))?;
        plane.offset = -plane.normal.dot(point.as_dvec3());
        Some(plane)
    }

    /// Returns the unit normal.
    #[must_use]
    pub fn normal(self) -> Vec3 {
        self.normal.as_vec3()
    }

    /// Returns the constant term in the plane equation.
    #[must_use]
    pub fn offset(self) -> f64 {
        self.offset
    }

    /// Returns the distance to the plane, positive along the normal.
    #[must_use]
    pub fn signed_distance(self, point: Vec3) -> f64 {
        self.signed_distance_f64(point.as_dvec3())
    }

    pub(crate) fn from_equation(equation: DVec4) -> Option<Self> {
        let normal = equation.truncate();
        let length = normal.length();
        if !equation.is_finite() || !length.is_finite() || length == 0.0 {
            return None;
        }

        Some(Self {
            normal: normal / length,
            offset: equation.w / length,
        })
    }

    pub(crate) fn normal_f64(self) -> DVec3 {
        self.normal
    }

    pub(crate) fn signed_distance_f64(self, point: DVec3) -> f64 {
        self.normal.dot(point) + self.offset
    }
}

impl Ray {
    /// Returns the first forward hit parameter for a plane.
    ///
    /// Returns zero when the origin lies on the plane, including coplanar rays.
    #[must_use]
    pub fn intersects_plane(self, plane: &Plane) -> Option<f32> {
        let (origin, direction) = self.finite_components()?;
        let distance = plane.signed_distance_f64(origin);
        if distance == 0.0 {
            return Some(0.0);
        }

        let rate = plane.normal.dot(direction);
        if rate == 0.0 {
            return None;
        }

        Self::forward_hit(-distance / rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_a_plane_and_preserves_its_point() {
        let point = Vec3::new(2.0, -1.0, 7.0);
        let plane = Plane::from_point_normal(point, Vec3::Y * 4.0).unwrap();

        assert_eq!(plane.normal(), Vec3::Y);
        assert_eq!(plane.offset(), 1.0);
        assert_eq!(plane.signed_distance(point), 0.0);
        assert_eq!(plane.signed_distance(point + Vec3::Y * 3.0), 3.0);
        assert_eq!(plane.signed_distance(point - Vec3::Y), -1.0);
    }

    #[test]
    fn rejects_zero_or_non_finite_plane_inputs() {
        assert_eq!(Plane::from_point_normal(Vec3::ZERO, Vec3::ZERO), None);
        assert_eq!(Plane::from_point_normal(Vec3::ZERO, Vec3::NAN), None);
        assert_eq!(Plane::from_point_normal(Vec3::NAN, Vec3::Y), None);
    }

    #[test]
    fn plane_hits_work_from_both_sides_at_non_unit_speed() {
        let plane = Plane::from_point_normal(Vec3::Y, Vec3::Y).unwrap();
        for (origin, direction) in [
            (Vec3::Y * 5.0, -Vec3::Y * 2.0),
            (-Vec3::Y * 3.0, Vec3::Y * 2.0),
        ] {
            assert_eq!(
                Ray { origin, direction }.intersects_plane(&plane),
                Some(2.0)
            );
        }
    }

    #[test]
    fn plane_handles_coplanar_parallel_and_backward_rays() {
        let plane = Plane::from_point_normal(Vec3::Y, Vec3::Y).unwrap();
        let coplanar = Ray {
            origin: Vec3::Y,
            direction: Vec3::X,
        };
        let parallel = Ray {
            origin: Vec3::ZERO,
            ..coplanar
        };
        let backward = Ray {
            direction: -Vec3::Y,
            ..parallel
        };

        assert_eq!(coplanar.intersects_plane(&plane), Some(0.0));
        assert_eq!(parallel.intersects_plane(&plane), None);
        assert_eq!(backward.intersects_plane(&plane), None);
    }
}
