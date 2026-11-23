use std::time::Duration;

pub(crate) const FIXED_HZ: u64 = 60;

/// Timing information published as an application resource.
///
/// Frame and fixed tick numbers start at zero before the first frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Time {
    pub(crate) frame: u64,
    pub(crate) elapsed: Duration,
    pub(crate) delta: Duration,
    pub(crate) fixed_ticks: u64,
    pub(crate) interpolation_alpha: f64,
}

impl Time {
    /// Duration of one 60 Hz step rounded to the nearest nanosecond.
    pub const FIXED_STEP: Duration =
        Duration::from_nanos((1_000_000_000 + FIXED_HZ / 2) / FIXED_HZ);

    /// One-based frame number during execution, or zero before the first frame.
    #[must_use]
    pub const fn frame(&self) -> u64 {
        self.frame
    }

    /// Total elapsed frame time supplied by the runner.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Delta for the current stage, using the fixed step during fixed update.
    #[must_use]
    pub const fn delta(&self) -> Duration {
        self.delta
    }

    /// Number of fixed steps entered since the application started.
    #[must_use]
    pub const fn fixed_ticks(&self) -> u64 {
        self.fixed_ticks
    }

    /// Fraction of the next fixed step accumulated for render interpolation.
    #[must_use]
    pub const fn interpolation_alpha(&self) -> f64 {
        self.interpolation_alpha
    }
}
