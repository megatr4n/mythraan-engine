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

    /// Returns whether the point lies inside or on the boundary.
    #[must_use]
    pub fn contains_point(self, point: Vec3) -> bool {
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }

    /// Returns the smallest box that encloses both boxes.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
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

    #[test]
    fn contains_point_includes_boundaries_and_checks_every_axis() {
        let bounds = Aabb {
            min: Vec3::splat(-1.0),
            max: Vec3::ONE,
        };

        for point in [Vec3::ZERO, Vec3::Y, bounds.min, bounds.max] {
            assert!(bounds.contains_point(point));
        }
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            assert!(!bounds.contains_point(axis * 2.0));
            assert!(!bounds.contains_point(axis * -2.0));
        }
    }

    #[test]
    fn contains_point_in_a_point_box_requires_exact_position() {
        let point = Vec3::ONE;
        let bounds = Aabb {
            min: point,
            max: point,
        };

        assert!(bounds.contains_point(point));
        assert!(!bounds.contains_point(point + Vec3::Z));
    }

    #[test]
    fn merge_encloses_disjoint_boxes_and_is_order_independent() {
        let first = Aabb {
            min: Vec3::new(-2.0, 1.0, -4.0),
            max: Vec3::new(-1.0, 2.0, 1.0),
        };
        let second = Aabb {
            min: Vec3::new(3.0, -1.0, -2.0),
            max: Vec3::new(4.0, 3.0, 2.0),
        };
        let expected = Aabb {
            min: Vec3::new(-2.0, -1.0, -4.0),
            max: Vec3::new(4.0, 3.0, 2.0),
        };

        assert_eq!(first.merge(second), expected);
        assert_eq!(second.merge(first), expected);
    }

    #[test]
    fn merge_with_an_enclosed_point_keeps_the_box() {
        let box_bounds = Aabb {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };
        let point = Aabb {
            min: Vec3::splat(0.5),
            max: Vec3::splat(0.5),
        };

        assert_eq!(box_bounds.merge(point), box_bounds);
    }
}
