//! Spheres in three dimensions.

use glam::Vec3;

/// A closed sphere with a finite center and a finite, nonnegative radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    /// Center in world coordinates.
    pub center: Vec3,
    /// Distance from the center to the surface.
    pub radius: f32,
}

impl Sphere {
    /// Checks whether a point lies inside or on the sphere.
    #[must_use]
    pub fn contains_point(self, point: Vec3) -> bool {
        let offset = point.as_dvec3() - self.center.as_dvec3();
        offset.length_squared() <= f64::from(self.radius).powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_contains_its_center_and_includes_its_surface() {
        let sphere = Sphere {
            center: Vec3::Y,
            radius: 2.0,
        };

        assert!(sphere.contains_point(Vec3::Y));
        assert!(sphere.contains_point(Vec3::Y + 2.0 * Vec3::X));
        assert!(!sphere.contains_point(Vec3::Y + 2.5 * Vec3::X));
        assert!(
            Sphere {
                radius: 0.0,
                ..sphere
            }
            .contains_point(Vec3::Y)
        );
    }
}
