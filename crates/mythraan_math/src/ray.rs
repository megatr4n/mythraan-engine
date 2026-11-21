//! Three-dimensional rays.

use glam::Vec3;

/// A half-line starting at an origin and extending along a direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Starting point of the ray.
    pub origin: Vec3,
    /// Finite, nonzero direction. Its length can differ from one.
    pub direction: Vec3,
}
