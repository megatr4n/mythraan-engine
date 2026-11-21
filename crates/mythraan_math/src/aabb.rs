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
