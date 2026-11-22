//! Generated checks for public math invariants.

use std::f32::consts::PI;
use std::ops::Range;

use mythraan_math::glam::{EulerRot, Quat, Vec3};
use mythraan_math::prelude::{Color, Transform};
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
}
