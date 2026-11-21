//! Axis-aligned bounding boxes.

use glam::Vec3;

/// A box with finite, componentwise ordered corners. Zero-width axes are valid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    /// Minimum coordinate on each axis.
    pub min: Vec3,
    /// Maximum coordinate on each axis.
    pub max: Vec3,
}

impl Aabb {
    /// Returns the midpoint between the corners.
    #[must_use]
    pub fn center(self) -> Vec3 {
        ((self.min.as_dvec3() + self.max.as_dvec3()) * 0.5).as_vec3()
    }

    /// Returns half the width, height, and depth.
    #[must_use]
    pub fn extents(self) -> Vec3 {
        ((self.max.as_dvec3() - self.min.as_dvec3()) * 0.5).as_vec3()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_is_midpoint_even_for_a_flat_box() {
        let bounds = Aabb {
            min: Vec3::new(-4.0, 2.0, -6.0),
            max: Vec3::new(2.0, 2.0, 8.0),
        };

        assert_eq!(bounds.center(), Vec3::new(-1.0, 2.0, 1.0));
    }

    #[test]
    fn center_of_large_coincident_corners_stays_finite() {
        let point = Vec3::splat(f32::MAX);
        let bounds = Aabb {
            min: point,
            max: point,
        };

        assert_eq!(bounds.center(), point);
    }

    #[test]
    fn extents_are_half_sizes_with_zero_for_a_flat_axis() {
        let bounds = Aabb {
            min: Vec3::new(-4.0, 2.0, -6.0),
            max: Vec3::new(2.0, 2.0, 8.0),
        };

        assert_eq!(bounds.extents(), Vec3::new(3.0, 0.0, 7.0));
    }

    #[test]
    fn extents_are_finite_across_the_f32_range() {
        let limit = Vec3::splat(f32::MAX);
        let bounds = Aabb {
            min: -limit,
            max: limit,
        };

        assert_eq!(bounds.extents(), limit);
    }
}
