//! View frusta and volume tests.

use glam::{DVec4, Mat4, Vec3};

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
    fn perspective_contains_visible_points() {
        let frustum = perspective();

        for point in [
            Vec3::new(3.0, 0.0, -4.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, -9.0),
        ] {
            assert!(frustum.contains_point(point), "{point:?}");
        }
    }
}
