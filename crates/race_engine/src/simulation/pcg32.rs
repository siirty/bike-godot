use crate::simulation::error::{RaceSimulationError, Result, SimulationValidationCode as C};
use crate::simulation::validate::check_positive_int;

/// A serializable position of a [`Pcg32`] generator in its stream. Carried in `RaceState` so a
/// resumed race continues the same sequence.
///
/// The algorithm ID is stored, not assumed. A change to the algorithm or the seed→state mapping
/// needs a new ID; `pcg32-v1` is never redefined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RandomState {
    state: u64,
    stream: u64,
}

impl RandomState {
    /// Validates the algorithm ID and an odd stream.
    pub fn new(algorithm: &str, state: u64, stream: u64) -> Result<Self> {
        if algorithm != Pcg32::ALGORITHM_ID {
            return Err(RaceSimulationError::validation(
                C::UnsupportedRandomAlgorithm,
                format!(
                    "unsupported RNG algorithm \"{algorithm}\"; this build implements \"{}\"",
                    Pcg32::ALGORITHM_ID
                ),
                "algorithm",
            ));
        }
        if stream & 1 == 0 {
            return Err(RaceSimulationError::validation(
                C::InvalidRandomStream,
                format!(
                    "the PCG stream/increment must be odd, got 0x{}",
                    hex64(stream)
                ),
                "stream",
            ));
        }
        Ok(Self { state, stream })
    }

    /// Always [`Pcg32::ALGORITHM_ID`] in this build.
    pub fn algorithm(&self) -> &'static str {
        Pcg32::ALGORITHM_ID
    }

    /// The 64-bit LCG state.
    pub fn state(&self) -> u64 {
        self.state
    }

    /// The 64-bit stream selector (PCG's `inc`), always odd.
    pub fn stream(&self) -> u64 {
        self.stream
    }
}

/// PCG-XSH-RR 64/32: the simulation's only source of randomness (`pcg_basic`'s
/// `pcg32_random_r` / `pcg32_srandom_r`). Draws happen in canonical `RiderId` order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    stream: u64,
}

impl Pcg32 {
    /// The identifier written into every [`RandomState`].
    pub const ALGORITHM_ID: &'static str = "pcg32-v1";
    const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

    /// A generator positioned at `state`.
    pub fn from_state(state: RandomState) -> Self {
        Self {
            state: state.state,
            stream: state.stream,
        }
    }

    /// The reference seeding procedure: zero the state, `inc = (stream << 1) | 1`, step, add the
    /// seed, step. `seed` and `stream` are 64-bit patterns (the prototype's signed Dart ints).
    pub fn seeded(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            stream: (stream << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// The current position, ready to snapshot.
    pub fn state(&self) -> RandomState {
        RandomState {
            state: self.state,
            stream: self.stream,
        }
    }

    /// The next value in `[0, 2^32)`.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(Self::MULTIPLIER).wrapping_add(self.stream);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// A value in `[0, bound)` without modulo bias. `bound` must be in `1..=2^32`.
    pub fn next_int_below(&mut self, bound: u64) -> Result<u64> {
        check_positive_int(bound as i64, "bound")?;
        if bound > 1 << 32 {
            return Err(RaceSimulationError::validation(
                C::OutOfRange,
                format!("bound must be at most 2^32, got {bound}"),
                "bound",
            ));
        }
        let threshold = ((1u64 << 32) - bound) % bound;
        loop {
            let r = u64::from(self.next_u32());
            if r >= threshold {
                return Ok(r % bound);
            }
        }
    }

    /// A double in `[0, 1)` with 53 significant bits, from two draws.
    pub fn next_f64(&mut self) -> f64 {
        let hi = f64::from(self.next_u32() >> 5);
        let lo = f64::from(self.next_u32() >> 6);
        (hi * 67_108_864.0 + lo) / 9_007_199_254_740_992.0
    }
}

/// Exactly 16 lowercase hex digits: how RNG state is written to a snapshot (a JSON number would
/// not survive a double).
pub fn hex64(value: u64) -> String {
    format!("{value:016x}")
}

/// Parses exactly 16 lowercase hex digits; anything else (prefix, upper case, short) is `None`,
/// so one value has one encoding.
pub fn parse_hex64(text: &str) -> Option<u64> {
    if text.len() != 16
        || !text
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return None;
    }
    u64::from_str_radix(text, 16).ok()
}
