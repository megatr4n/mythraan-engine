//! Axis-aligned bounding boxes.

use glam::{DVec3, Mat4, Vec3};

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

    /// Bounds a set of points. Returns `None` for empty input or non-finite points.
    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next().filter(|point| point.is_finite())?;
        let mut bounds = Self {
            min: first,
            max: first,
        };

        for point in points {
            if !point.is_finite() {
                return None;
            }
            bounds.min = bounds.min.min(point);
            bounds.max = bounds.max.max(point);
        }

        Some(bounds)
    }

    /// Bounds this box after an affine transformation.
    ///
    /// Requires a finite affine matrix and transformed bounds within the `f32` range.
    #[must_use]
    pub fn transformed(self, matrix: &Mat4) -> Self {
        let axes = [
            (matrix.x_axis.truncate(), self.min.x, self.max.x),
            (matrix.y_axis.truncate(), self.min.y, self.max.y),
            (matrix.z_axis.truncate(), self.min.z, self.max.z),
        ];
        let mut min = DVec3::ZERO;
        let mut max = DVec3::ZERO;
        for (axis, lower, upper) in axes {
            let axis = axis.as_dvec3();
            let lower = axis * f64::from(lower);
            let upper = axis * f64::from(upper);
            min += lower.min(upper);
            max += lower.max(upper);
        }

        let translation = matrix.w_axis.truncate().as_dvec3();
        Self {
            min: (min + translation).as_vec3(),
            max: (max + translation).as_vec3(),
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

    #[test]
    fn from_points_finds_tight_bounds_for_an_unordered_cloud() {
        let points = [
            Vec3::new(2.0, -1.0, 4.0),
            Vec3::new(-5.0, 3.0, 2.0),
            Vec3::new(1.0, 1.0, -6.0),
        ];
        let expected = Aabb {
            min: Vec3::new(-5.0, -1.0, -6.0),
            max: Vec3::new(2.0, 3.0, 4.0),
        };

        assert_eq!(Aabb::from_points(points), Some(expected));
    }

    #[test]
    fn from_points_with_one_point_creates_a_degenerate_box() {
        let point = Vec3::new(-2.0, 4.0, 6.0);

        assert_eq!(
            Aabb::from_points([point]),
            Some(Aabb {
                min: point,
                max: point
            })
        );
    }

    #[test]
    fn from_points_rejects_empty_and_non_finite_input() {
        assert_eq!(Aabb::from_points([]), None);
        assert_eq!(Aabb::from_points([Vec3::NAN, Vec3::ZERO]), None);
        assert_eq!(
            Aabb::from_points([Vec3::ZERO, Vec3::splat(f32::INFINITY)]),
            None
        );
    }

    #[test]
    fn transformed_bounds_rotation_reflection_scale_and_translation() {
        let bounds = Aabb {
            min: Vec3::new(-1.0, 1.0, 0.0),
            max: Vec3::new(2.0, 4.0, 2.0),
        };
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::new(-2.0, 3.0, 0.5),
            glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Vec3::new(10.0, 2.0, -1.0),
        );
        let transformed = bounds.transformed(&matrix);

        assert!(
            transformed
                .min
                .abs_diff_eq(Vec3::new(-2.0, -2.0, -1.0), 1.0e-5)
        );
        assert!(
            transformed
                .max
                .abs_diff_eq(Vec3::new(7.0, 4.0, 0.0), 1.0e-5)
        );
        assert_eq!(bounds.transformed(&Mat4::IDENTITY), bounds);
    }

    #[test]
    fn transformed_bounds_a_sheared_flat_box() {
        let bounds = Aabb {
            min: Vec3::new(-2.0, -1.0, 0.0),
            max: Vec3::new(1.0, 3.0, 0.0),
        };
        let matrix = Mat4 {
            y_axis: glam::Vec4::new(2.0, 1.0, 0.0, 0.0),
            ..Mat4::IDENTITY
        };

        assert_eq!(
            bounds.transformed(&matrix),
            Aabb {
                min: Vec3::new(-4.0, -1.0, 0.0),
                max: Vec3::new(7.0, 3.0, 0.0),
            }
        );
    }

    #[test]
    fn transformed_avoids_overflow_when_large_terms_cancel() {
        let point = Vec3::new(f32::MAX, f32::MAX, 0.0);
        let bounds = Aabb {
            min: point,
            max: point,
        };
        let matrix = Mat4 {
            x_axis: glam::Vec4::new(2.0, 0.0, 0.0, 0.0),
            y_axis: glam::Vec4::new(-2.0, 1.0, 0.0, 0.0),
            ..Mat4::IDENTITY
        };
        let expected = Vec3::new(0.0, f32::MAX, 0.0);

        assert_eq!(
            bounds.transformed(&matrix),
            Aabb {
                min: expected,
                max: expected
            }
        );
    }
}
