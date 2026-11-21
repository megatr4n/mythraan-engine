//! Local and world-space transforms.

use glam::{Mat3, Mat4, Quat, Vec3, Vec4};

/// A local transform that applies scale, rotation, and then translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Offset in the parent coordinate space.
    pub translation: Vec3,
    /// Orientation relative to the parent, represented by a unit quaternion.
    pub rotation: Quat,
    /// Scale along the local axes.
    pub scale: Vec3,
}

impl Transform {
    /// Leaves points and directions unchanged.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Converts the local transform to an affine matrix.
    #[must_use]
    pub fn to_matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A world-space transform stored as a matrix so it can represent shear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform(pub Mat4);

impl GlobalTransform {
    /// Leaves points and directions unchanged.
    pub const IDENTITY: Self = Self(Mat4::IDENTITY);

    /// Decomposes the matrix into scale, rotation, and translation.
    ///
    /// Returns `None` for non-finite or non-affine matrices, zero-length axes, or
    /// shear beyond floating-point tolerance. A reflection uses a negative X scale.
    #[must_use]
    pub fn try_to_transform(self) -> Option<Transform> {
        let matrix = self.0;
        if !matrix.is_finite() || matrix.row(3) != Vec4::W {
            return None;
        }

        let x = matrix.x_axis.truncate().as_dvec3();
        let y = matrix.y_axis.truncate().as_dvec3();
        let z = matrix.z_axis.truncate().as_dvec3();
        let mut scale_x = x.length();
        let scale_y = y.length();
        let scale_z = z.length();
        if scale_x == 0.0 || scale_y == 0.0 || scale_z == 0.0 {
            return None;
        }

        let mut x = x / scale_x;
        let y = y / scale_y;
        let z = z / scale_z;
        let tolerance = 8.0 * f64::from(f32::EPSILON);
        if x.dot(y).abs() > tolerance || x.dot(z).abs() > tolerance || y.dot(z).abs() > tolerance {
            return None;
        }

        if x.cross(y).dot(z) < 0.0 {
            scale_x = -scale_x;
            x = -x;
        }

        let scale = Vec3::new(scale_x as f32, scale_y as f32, scale_z as f32);
        if !scale.is_finite() {
            return None;
        }

        let basis = Mat3::from_cols(x.as_vec3(), y.as_vec3(), z.as_vec3());
        Some(Transform {
            translation: matrix.w_axis.truncate(),
            rotation: Quat::from_mat3(&basis).normalize(),
            scale,
        })
    }
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<Transform> for GlobalTransform {
    fn from(transform: Transform) -> Self {
        Self(transform.to_matrix())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: f32 = 1.0e-5;

    #[test]
    fn identity_preserves_transforms() {
        assert_eq!(Transform::default().to_matrix(), Mat4::IDENTITY);
        assert_eq!(
            GlobalTransform::default().try_to_transform(),
            Some(Transform::IDENTITY)
        );
    }

    #[test]
    fn decomposition_round_trips_mirrored_transforms() {
        let transform = Transform {
            translation: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_rotation_z(0.4),
            scale: Vec3::new(2.0, -3.0, 4.0),
        };
        let global = GlobalTransform::from(transform);
        let decomposed = global.try_to_transform().unwrap();

        assert!(decomposed.to_matrix().abs_diff_eq(global.0, TOLERANCE));
    }
}
