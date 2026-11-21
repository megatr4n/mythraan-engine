//! Three-dimensional rays.

use glam::{DVec3, Vec3};

use crate::aabb::Aabb;

/// A half-line starting at an origin and extending along a direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Starting point of the ray.
    pub origin: Vec3,
    /// Finite, nonzero direction. Its length can differ from one.
    pub direction: Vec3,
}

impl Ray {
    /// Returns the first hit parameter `t` in `origin + t * direction`.
    ///
    /// Returns zero when the origin is inside or on the box. Misses, invalid
    /// rays, and hits whose parameter overflows `f32` return `None`.
    #[must_use]
    pub fn intersects_aabb(self, bounds: &Aabb) -> Option<f32> {
        let (origin, direction) = self.finite_components()?;
        Self::intersect_slabs(
            origin,
            direction,
            bounds.min.as_dvec3(),
            bounds.max.as_dvec3(),
        )
    }

    pub(crate) fn finite_components(self) -> Option<(DVec3, DVec3)> {
        if !self.origin.is_finite() || !self.direction.is_finite() || self.direction == Vec3::ZERO {
            return None;
        }

        Some((self.origin.as_dvec3(), self.direction.as_dvec3()))
    }

    pub(crate) fn forward_hit(parameter: f64) -> Option<f32> {
        let hit = parameter as f32;
        (parameter >= 0.0 && hit.is_finite()).then_some(hit)
    }

    pub(crate) fn intersect_slabs(
        origin: DVec3,
        direction: DVec3,
        min: DVec3,
        max: DVec3,
    ) -> Option<f32> {
        let slabs = [
            (origin.x, direction.x, min.x, max.x),
            (origin.y, direction.y, min.y, max.y),
            (origin.z, direction.z, min.z, max.z),
        ];
        let mut entry = 0.0_f64;
        let mut exit = f64::INFINITY;
        for (origin, direction, min, max) in slabs {
            if direction == 0.0 {
                if !(min..=max).contains(&origin) {
                    return None;
                }
                continue;
            }

            let first = (min - origin) / direction;
            let second = (max - origin) / direction;
            entry = entry.max(first.min(second));
            exit = exit.min(first.max(second));
            if entry > exit {
                return None;
            }
        }

        Self::forward_hit(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: Aabb = Aabb {
        min: Vec3::ZERO,
        max: Vec3::ONE,
    };

    #[test]
    fn hits_with_a_negative_non_unit_direction() {
        let ray = Ray {
            origin: Vec3::new(5.0, 0.5, 0.5),
            direction: -2.0 * Vec3::X,
        };

        assert_eq!(ray.intersects_aabb(&BOX), Some(2.0));
    }

    #[test]
    fn misses_when_the_slabs_do_not_overlap() {
        let ray = Ray {
            origin: Vec3::new(-2.0, -4.0, 0.5),
            direction: Vec3::new(1.0, 1.0, 0.0),
        };

        assert_eq!(ray.intersects_aabb(&BOX), None);
    }

    #[test]
    fn misses_a_box_behind_the_origin() {
        let ray = Ray {
            origin: Vec3::new(2.0, 0.5, 0.5),
            direction: Vec3::X,
        };

        assert_eq!(ray.intersects_aabb(&BOX), None);
    }

    #[test]
    fn starts_inside_or_on_the_boundary_at_zero() {
        for origin in [Vec3::splat(0.5), Vec3::ONE] {
            let ray = Ray {
                origin,
                direction: Vec3::X,
            };

            assert_eq!(ray.intersects_aabb(&BOX), Some(0.0));
        }
    }

    #[test]
    fn parallel_ray_hits_on_a_slab_boundary_and_misses_outside_it() {
        let mut ray = Ray {
            origin: Vec3::new(-2.0, 0.0, 0.5),
            direction: Vec3::new(1.0, -0.0, 0.0),
        };

        assert_eq!(ray.intersects_aabb(&BOX), Some(2.0));

        ray.origin.y = -1.0;
        assert_eq!(ray.intersects_aabb(&BOX), None);
    }

    #[test]
    fn can_hit_a_box_with_zero_size() {
        let ray = Ray {
            origin: -Vec3::ONE,
            direction: Vec3::ONE,
        };
        let point = Aabb {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
        };

        assert_eq!(ray.intersects_aabb(&point), Some(1.0));
    }

    #[test]
    fn handles_extreme_finite_coordinates_and_directions() {
        let point = Aabb {
            min: Vec3::X * f32::MAX,
            max: Vec3::X * f32::MAX,
        };
        let ray = Ray {
            origin: -Vec3::X * f32::MAX,
            direction: Vec3::X * f32::MAX,
        };

        assert_eq!(ray.intersects_aabb(&point), Some(2.0));

        let mut tiny = Ray {
            origin: Vec3::new(-1.0e-40, 0.5, 0.5),
            direction: Vec3::X * 1.0e-40,
        };
        assert_eq!(tiny.intersects_aabb(&BOX), Some(1.0));

        tiny.origin.x = -1.0;
        assert_eq!(tiny.intersects_aabb(&BOX), None);
    }

    #[test]
    fn rejects_invalid_rays() {
        for direction in [Vec3::ZERO, Vec3::NAN, Vec3::splat(f32::INFINITY)] {
            let ray = Ray {
                origin: Vec3::splat(0.5),
                direction,
            };
            assert_eq!(ray.intersects_aabb(&BOX), None);
        }
        let ray = Ray {
            origin: Vec3::NAN,
            direction: Vec3::X,
        };
        assert_eq!(ray.intersects_aabb(&BOX), None);
    }
}
