//! Generated checks for public math invariants.

use std::f32::consts::PI;
use std::ops::Range;

use mythraan_math::glam::{EulerRot, Quat, Vec3};
use mythraan_math::prelude::{Aabb, Bvh, Color, Ray, Transform};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

fn vector(range: Range<f32>) -> impl Strategy<Value = Vec3> {
    (range.clone(), range.clone(), range).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

fn transform() -> impl Strategy<Value = Transform> {
    (
        vector(-32.0..32.0),
        vector(-PI..PI),
        vector(0.25..4.0),
        any::<(bool, bool, bool)>(),
    )
        .prop_map(|(translation, angles, scale, (x, y, z))| {
            let sign = |negative| if negative { -1.0 } else { 1.0 };
            Transform {
                translation,
                rotation: Quat::from_euler(EulerRot::XYZ, angles.x, angles.y, angles.z),
                scale: scale * Vec3::new(sign(x), sign(y), sign(z)),
            }
        })
}

fn bounds() -> impl Strategy<Value = Aabb> {
    (vector(-32.0..32.0), vector(0.0..8.0)).prop_map(|(center, half)| Aabb {
        min: center - half,
        max: center + half,
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(0x4d59_5448_5241_414e),
        ..ProptestConfig::default()
    })]

    #[test]
    fn inverse_restores_points_after_signed_scale(transform in transform(), point in vector(-8.0..8.0)) {
        let world = transform.to_matrix().transform_point3(point);
        let inverse = transform.inverse().unwrap();
        let restored = inverse.0.transform_point3(world);
        prop_assert!(point.abs_diff_eq(restored, 0.0001), "{point:?} vs {restored:?}");
    }

    #[test]
    fn composition_matches_sequential_application(
        parent in transform(),
        child in transform(),
        point in vector(-8.0..8.0),
    ) {
        let expected = parent.to_matrix().transform_point3(child.to_matrix().transform_point3(point));
        let actual = parent.compose(child).0.transform_point3(point);
        prop_assert!(expected.abs_diff_eq(actual, 0.0002), "{expected:?} vs {actual:?}");
    }

    #[test]
    fn srgb_round_trip_preserves_channels_and_alpha(rgba in prop::array::uniform4(0.0f32..1.0)) {
        let [r, g, b, a] = Color::from_srgba(rgba).to_srgba();
        for (actual, expected) in [r, g, b].into_iter().zip(rgba) {
            prop_assert!((actual - expected).abs() <= 0.000001, "{actual} vs {expected}");
        }
        prop_assert_eq!(a.to_bits(), rgba[3].to_bits());
    }

    #[test]
    fn merged_bounds_contain_both_boxes(left in bounds(), right in bounds()) {
        let merged = left.merge(right);
        prop_assert_eq!(merged, right.merge(left));
        for point in [left.min, left.max, right.min, right.max] {
            prop_assert!(merged.contains_point(point));
        }
    }

    #[test]
    fn transformed_bounds_match_a_reference_box_from_eight_corners(
        bounds in bounds(),
        transform in transform(),
    ) {
        let matrix = transform.to_matrix();
        let mut corners = Vec::with_capacity(8);
        for x in [bounds.min.x, bounds.max.x] {
            for y in [bounds.min.y, bounds.max.y] {
                for z in [bounds.min.z, bounds.max.z] {
                    corners.push(matrix.transform_point3(Vec3::new(x, y, z)));
                }
            }
        }
        let expected = Aabb::from_points(corners).unwrap();
        let actual = bounds.transformed(&matrix);
        prop_assert!(actual.min.abs_diff_eq(expected.min, 0.0001));
        prop_assert!(actual.max.abs_diff_eq(expected.max, 0.0001));
    }

    #[test]
    fn axis_aligned_ray_hits_the_near_face_at_the_expected_parameter(
        bounds in bounds(),
        offset in 0.5f32..20.0,
        speed in 0.25f32..4.0,
    ) {
        let origin = Vec3::new(bounds.min.x - offset, bounds.center().y, bounds.center().z);
        let ray = Ray { origin, direction: Vec3::X * speed };
        let expected = (bounds.min.x - origin.x) / speed;
        let actual = ray.intersects_aabb(&bounds).unwrap();
        prop_assert!((actual - expected).abs() <= expected.abs().max(1.0) * 0.000001);
    }

    #[test]
    fn bvh_overlap_query_matches_a_linear_scan(
        boxes in prop::collection::vec(bounds(), 0..48),
        query in bounds(),
    ) {
        let items: Vec<_> = boxes.into_iter().zip(0..).collect();
        let expected: Vec<_> = items.iter()
            .filter(|(candidate, _)| {
                candidate.min.cmple(query.max).all() && query.min.cmple(candidate.max).all()
            })
            .map(|(_, id)| *id)
            .collect();
        let mut actual = Bvh::build(items).query_aabb(&query);
        actual.sort_unstable();
        prop_assert_eq!(actual, expected);
    }
}
