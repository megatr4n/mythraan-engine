//! Seeded pseudorandom sampling.

/// A reproducible SplitMix64 generator for simulations and procedural content.
///
/// The same seed and sequence of calls reproduce the same output.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Initializes the generator with any 64-bit seed.
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64-bit sample.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mixed = (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_reference_sequences() {
        for (seed, expected) in [
            (
                0,
                [
                    0xe220_a839_7b1d_cdaf,
                    0x6e78_9e6a_a1b9_65f4,
                    0x06c4_5d18_8009_454f,
                    0xf88b_b8a8_724c_81ec,
                ],
            ),
            (
                u64::MAX,
                [
                    0xe4d9_7177_1b65_2c20,
                    0xe99f_f867_dbf6_82c9,
                    0x382f_f84c_b272_81e9,
                    0x6d1d_b36c_cba9_82d2,
                ],
            ),
        ] {
            let mut rng = Rng::from_seed(seed);
            assert_eq!(expected.map(|_| rng.next_u64()), expected);
        }
    }

    #[test]
    fn cloning_reproduces_the_remaining_stream() {
        let mut rng = Rng::from_seed(81);
        rng.next_u64();
        let mut replay = rng.clone();

        for _ in 0..16 {
            assert_eq!(rng.next_u64(), replay.next_u64());
        }
    }
}
