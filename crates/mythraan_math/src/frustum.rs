//! View frusta and volume tests.

use glam::{DVec3, DVec4, Mat4, Vec3};

use crate::aabb::Aabb;

/// A view volume bounded by six inward-facing planes with unit normals.
#[derive(Clone, Debug, PartialEq)]
pub struct Frustum {
    planes: [DVec4; 6],
}

impl Frustum {
    /// Extracts world-space planes from a view-projection matrix with NDC depth in `[0, 1]`.
    ///
    /// Returns `None` for a non-finite matrix or a plane with a zero normal.
    #[must_use]
    pub fn from_view_projection(view_projection: &Mat4) -> Option<Self> {
        if !view_projection.is_finite() {
            return None;
        }

        let matrix = view_projection.as_dmat4();
        let x = matrix.row(0);
        let y = matrix.row(1);
        let z = matrix.row(2);
        let w = matrix.row(3);
        let mut planes = [w + x, w - x, w + y, w - y, z, w - z];
        for plane in &mut planes {
            let length = plane.truncate().length();
            if length == 0.0 {
                return None;
            }
            *plane /= length;
        }

        Some(Self { planes })
    }

    /// Returns whether a world-space point lies inside or on the boundary.
    #[must_use]
    pub fn contains_point(&self, point: Vec3) -> bool {
        let point = point.as_dvec3().extend(1.0);
        self.planes.iter().all(|plane| plane.dot(point) >= 0.0)
    }

    /// Returns false when any frustum plane excludes the entire world-space box.
    ///
    /// A true result includes boundary contact and can include disjoint boxes
    /// near frustum edges.
    #[must_use]
    pub fn intersects_aabb(&self, bounds: &Aabb) -> bool {
        let min = bounds.min.as_dvec3();
        let max = bounds.max.as_dvec3();
        self.planes.iter().all(|plane| {
            let normal = plane.truncate();
            let furthest = DVec3::select(normal.cmpge(DVec3::ZERO), max, min);
            plane.dot(furthest.extend(1.0)) >= 0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn perspective() -> Frustum {
        let projection = Mat4::perspective_rh(FRAC_PI_2, 1.0, 1.0, 9.0);
        Frustum::from_view_projection(&projection).unwrap()
    }

    #[test]
    fn perspective_planes_have_unit_normals() {
        let frustum = perspective();

        for plane in frustum.planes {
            assert!((plane.truncate().length() - 1.0).abs() < 1.0e-12);
        }
    }

    #[test]
    fn perspective_clips_points_against_each_plane() {
        let frustum = perspective();

        for point in [
            Vec3::new(3.0, 0.0, -4.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, -9.0),
        ] {
            assert!(frustum.contains_point(point), "{point:?}");
        }
        for point in [
            Vec3::new(-5.0, 0.0, -4.0),
            Vec3::new(5.0, 0.0, -4.0),
            Vec3::new(0.0, -5.0, -4.0),
            Vec3::new(0.0, 5.0, -4.0),
            Vec3::new(0.0, 0.0, -0.75),
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
        ] {
            assert!(!frustum.contains_point(point), "{point:?}");
        }
    }

    #[test]
    fn perspective_keeps_a_box_crossing_a_side_plane() {
        let frustum = perspective();
        let crossing = Aabb {
            min: Vec3::new(3.75, -0.5, -4.25),
            max: Vec3::new(5.0, 0.5, -3.75),
        };
        let outside = Aabb {
            min: Vec3::new(3.75, -0.5, -2.25),
            max: Vec3::new(5.0, 0.5, -1.75),
        };

        assert!(frustum.intersects_aabb(&crossing));
        assert!(!frustum.intersects_aabb(&outside));
    }
}
