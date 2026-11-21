//! Linear colors and sRGB conversion.

/// Linear RGB with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color([f32; 4]);

impl Color {
    /// Stores linear red, green, blue, and alpha without clamping.
    #[must_use]
    pub const fn from_linear_rgba(rgba: [f32; 4]) -> Self {
        Self(rgba)
    }

    /// Returns linear red, green, blue, and alpha.
    #[must_use]
    pub const fn to_linear_rgba(self) -> [f32; 4] {
        self.0
    }

    /// Decodes sRGB channels into linear light, preserving alpha.
    #[must_use]
    pub fn from_srgba([red, green, blue, alpha]: [f32; 4]) -> Self {
        Self([
            decode_srgb(red),
            decode_srgb(green),
            decode_srgb(blue),
            alpha,
        ])
    }

    /// Encodes the RGB channels as sRGB without clamping or changing alpha.
    #[must_use]
    pub fn to_srgba(self) -> [f32; 4] {
        let [red, green, blue, alpha] = self.0;
        [
            encode_srgb(red),
            encode_srgb(green),
            encode_srgb(blue),
            alpha,
        ]
    }
}

fn decode_srgb(channel: f32) -> f32 {
    let magnitude = channel.abs();
    if magnitude <= 0.04045 {
        channel / 12.92
    } else {
        ((magnitude + 0.055) / 1.055).powf(2.4).copysign(channel)
    }
}

fn encode_srgb(channel: f32) -> f32 {
    let magnitude = channel.abs();
    if magnitude <= 0.0031308 {
        channel * 12.92
    } else {
        (1.055 * magnitude.powf(1.0 / 2.4) - 0.055).copysign(channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_channels_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1.0e-6, "{actual} vs {expected}");
        }
    }

    #[test]
    fn decodes_reference_srgb_values() {
        let linear = Color::from_srgba([0.0, 0.5, 1.0, 0.25]).to_linear_rgba();
        assert_channels_close(linear, [0.0, 0.214_041_14, 1.0, 0.25]);
    }

    #[test]
    fn round_trips_srgb_across_both_branches_and_preserves_alpha() {
        let srgb = [0.01, 0.04045, 0.8, 0.35];
        let color = Color::from_srgba(srgb);

        assert_channels_close(color.to_srgba(), srgb);
        assert_eq!(color.to_srgba()[3], srgb[3]);
    }

    #[test]
    fn round_trips_linear_values_without_clamping() {
        let linear = [-0.25, 0.002, 2.0, 0.75];
        let color = Color::from_linear_rgba(linear);

        assert_eq!(color.to_linear_rgba(), linear);
        assert_channels_close(Color::from_srgba(color.to_srgba()).to_linear_rgba(), linear);
    }
}
