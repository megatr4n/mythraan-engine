//! Timing resource defaults and fixed-step execution.

use std::time::Duration;

use mythraan_app::prelude::Time;

#[test]
fn time_before_the_first_frame_starts_at_zero() {
    let time = Time::default();
    assert_eq!(time.frame(), 0);
    assert_eq!(time.fixed_ticks(), 0);
    assert_eq!(time.elapsed(), Duration::ZERO);
    assert_eq!(time.delta(), Duration::ZERO);
    assert_eq!(time.interpolation_alpha(), 0.0);
    assert_eq!(Time::FIXED_STEP, Duration::from_nanos(16_666_667));
}
