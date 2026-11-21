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

    /// Applies `child` first, then this transform, preserving any resulting shear.
    ///
    /// ```
    /// use mythraan_math::{glam::Vec3, prelude::Transform};
    ///
    /// let parent = Transform {
    ///     scale: Vec3::splat(2.0),
    ///     ..Transform::IDENTITY
    /// };
    /// let child = Transform {
    ///     translation: Vec3::X,
    ///     ..Transform::IDENTITY
    /// };
    /// assert_eq!(parent.compose(child).0.transform_point3(Vec3::ZERO), Vec3::X * 2.0);
    /// ```
    #[must_use]
    pub fn compose(self, child: Self) -> GlobalTransform {
        GlobalTransform::from(self).compose(child.into())
    }

    /// Returns the inverse matrix, or `None` if no finite inverse can be represented.
    #[must_use]
    pub fn inverse(self) -> Option<GlobalTransform> {
        GlobalTransform::from(self).inverse()
    }

    /// Points local `-Z` at `target` and aligns local `+Y` with the projected `up`.
    ///
    /// The target and up direction are in parent coordinates. Translation and scale
    /// are preserved. Returns `None` if the inputs cannot define an orientation.
    #[must_use]
    pub fn looking_at(mut self, target: Vec3, up: Vec3) -> Option<Self> {
        let forward = (target.as_dvec3() - self.translation.as_dvec3()).try_normalize()?;
        let up = up.as_dvec3().try_normalize()?;
        let right = forward.cross(up);
        if right.length_squared() <= f64::EPSILON {
            return None;
        }

        let right = right.normalize();
        let back = -forward;
        let corrected_up = back.cross(right);
        let basis = Mat3::from_cols(right.as_vec3(), corrected_up.as_vec3(), back.as_vec3());
        self.rotation = Quat::from_mat3(&basis).normalize();
        Some(self)
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

    /// Applies `child` first, then this transform.
    #[must_use]
    pub fn compose(self, child: Self) -> Self {
        Self(self.0 * child.0)
    }

    /// Returns the inverse matrix, or `None` if no finite inverse can be represented.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        // A determinant in f32 can underflow or overflow even for invertible scales.
        let matrix = self.0.as_dmat4();
        let determinant = matrix.determinant();
        if determinant == 0.0 || !determinant.is_finite() {
            return None;
        }

        let inverse = matrix.inverse().as_mat4();
        inverse.is_finite().then_some(Self(inverse))
    }

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
        let transform = Transform {
            translation: Vec3::ONE,
            ..Transform::IDENTITY
        };
        let global = GlobalTransform::from(transform);

        assert_eq!(Transform::default().to_matrix(), Mat4::IDENTITY);
        assert_eq!(
            GlobalTransform::default().try_to_transform(),
            Some(Transform::IDENTITY)
        );
        assert_eq!(Transform::IDENTITY.compose(transform), global);
        assert_eq!(global.compose(GlobalTransform::IDENTITY), global);
    }

    #[test]
    fn composition_applies_child_before_parent() {
        let parent = Transform {
            translation: Vec3::new(4.0, 5.0, 6.0),
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            scale: Vec3::new(2.0, 3.0, 4.0),
        };
        let child = Transform {
            translation: Vec3::X,
            ..Transform::IDENTITY
        };

        let child_origin = parent.compose(child).0.transform_point3(Vec3::ZERO);
        assert!(child_origin.abs_diff_eq(Vec3::new(4.0, 7.0, 6.0), TOLERANCE));
    }

    #[test]
    fn inverse_round_trip_with_rotation_and_nonuniform_scale() {
        let transform = Transform {
            translation: Vec3::new(-4.0, 2.0, 8.0),
            rotation: Quat::from_rotation_y(0.8),
            scale: Vec3::new(2.0, 3.0, 0.5),
        };
        let global = GlobalTransform::from(transform);
        let inverse = transform.inverse().unwrap();
        let point = Vec3::new(1.0, -2.0, 3.0);

        assert!(
            inverse
                .0
                .transform_point3(global.0.transform_point3(point))
                .abs_diff_eq(point, TOLERANCE)
        );
        assert!(
            global
                .compose(inverse)
                .0
                .abs_diff_eq(Mat4::IDENTITY, TOLERANCE)
        );
    }

    #[test]
    fn inverse_handles_finite_extreme_scales_and_rejects_zero_scale() {
        let singular = Transform {
            scale: Vec3::new(1.0, 0.0, 1.0),
            ..Transform::IDENTITY
        };
        assert_eq!(singular.inverse(), None);

        for scale in [1.0e-20, 1.0e20] {
            let transform = Transform {
                scale: Vec3::splat(scale),
                ..Transform::IDENTITY
            };
            let global = GlobalTransform::from(transform);
            let inverse = global.inverse().unwrap();
            assert!(
                inverse
                    .compose(global)
                    .0
                    .abs_diff_eq(Mat4::IDENTITY, TOLERANCE)
            );
        }
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

    #[test]
    fn composition_preserves_shear_that_local_transforms_cannot_store() {
        let parent = Transform {
            scale: Vec3::new(2.0, 3.0, 1.0),
            ..Transform::IDENTITY
        };
        let child = Transform {
            rotation: Quat::from_rotation_z(0.6),
            ..Transform::IDENTITY
        };
        let point = Vec3::new(1.0, 2.0, 3.0);
        let expected = parent
            .to_matrix()
            .transform_point3(child.to_matrix().transform_point3(point));
        let composed = parent.compose(child);

        assert!(
            composed
                .0
                .transform_point3(point)
                .abs_diff_eq(expected, TOLERANCE)
        );
        assert_eq!(composed.try_to_transform(), None);
    }

    #[test]
    fn looking_at_points_negative_z_at_the_target() {
        let transform = Transform {
            translation: Vec3::new(1.0, 2.0, 3.0),
            scale: Vec3::new(2.0, 3.0, 4.0),
            ..Transform::IDENTITY
        };
        let target = transform.translation + Vec3::X;
        let oriented = transform.looking_at(target, Vec3::Y).unwrap();

        assert!((oriented.rotation * Vec3::NEG_Z).abs_diff_eq(Vec3::X, TOLERANCE));
        assert!((oriented.rotation * Vec3::Y).abs_diff_eq(Vec3::Y, TOLERANCE));
        assert_eq!(oriented.translation, transform.translation);
        assert_eq!(oriented.scale, transform.scale);
        assert_eq!(transform.looking_at(transform.translation, Vec3::Y), None);
        assert_eq!(transform.looking_at(target, Vec3::X), None);
    }
}
