//! Spheres in three dimensions.

use glam::Vec3;

use crate::ray::Ray;

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

impl Ray {
    /// Returns the first forward hit parameter for a sphere.
    ///
    /// Returns zero when the origin is inside or on its surface.
    #[must_use]
    pub fn intersects_sphere(self, sphere: &Sphere) -> Option<f32> {
        let (origin, direction) = self.finite_components()?;
        let to_center = sphere.center.as_dvec3() - origin;
        let radius_squared = f64::from(sphere.radius).powi(2);
        let constant = to_center.length_squared() - radius_squared;
        if constant <= 0.0 {
            return Some(0.0);
        }

        let projected = to_center.dot(direction);
        if projected <= 0.0 {
            return None;
        }

        let direction_squared = direction.length_squared();
        let perpendicular_squared = to_center.cross(direction).length_squared() / direction_squared;
        if perpendicular_squared > radius_squared {
            return None;
        }

        let root = (direction_squared * (radius_squared - perpendicular_squared)).sqrt();
        // Use the larger root to avoid subtracting nearly equal values.
        Self::forward_hit(constant / (projected + root))
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

    #[test]
    fn sphere_reports_the_first_hit_and_returns_zero_from_inside() {
        let sphere = Sphere {
            center: Vec3::ZERO,
            radius: 1.0,
        };
        let ray = Ray {
            origin: -5.0 * Vec3::X,
            direction: 2.0 * Vec3::X,
        };
        let inside = Ray {
            origin: Vec3::ZERO,
            ..ray
        };

        assert_eq!(ray.intersects_sphere(&sphere), Some(2.0));
        assert_eq!(inside.intersects_sphere(&sphere), Some(0.0));
        assert_eq!(
            Ray {
                direction: -ray.direction,
                ..ray
            }
            .intersects_sphere(&sphere),
            None
        );
    }

    #[test]
    fn sphere_accepts_a_tangent_and_rejects_a_distant_near_miss() {
        let sphere = Sphere {
            center: Vec3::ZERO,
            radius: 1.0,
        };
        let tangent = Ray {
            origin: Vec3::new(-5.0, 1.0, 0.0),
            direction: Vec3::X,
        };
        let miss = Ray {
            origin: Vec3::new(-1.0e20, 2.0, 0.0),
            direction: Vec3::X,
        };

        assert_eq!(tangent.intersects_sphere(&sphere), Some(5.0));
        assert_eq!(miss.intersects_sphere(&sphere), None);
    }
}
