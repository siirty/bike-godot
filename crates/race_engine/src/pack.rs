//! Seeded pack pacing. Plans choose crank-power intentions; the kernel enforces physiology.
//! Plans observe only simulation time and their rider's reserve, never workout targets.
use crate::{
    Pcg32, RaceSimulationError, Result, RiderDefinition, RiderState, SimulationValidationCode as C,
};
use std::time::Duration;

/// A seeded repeating schedule, generated once before the race. The outer scenario retains
/// this immutable input policy; kernel snapshots still need the same future inputs to resume.
#[derive(Debug, Clone, PartialEq)]
pub struct PackPacePlan {
    fractions: Vec<f64>,
}
impl PackPacePlan {
    /// A 10-minute schedule of 30-second blocks. Mostly 85–100% CP with occasional
    /// 130–160% surges. Parameters are provisional fixture values, not calibrated tactics.
    pub fn seeded(seed: u64) -> Self {
        let mut rng = Pcg32::seeded(seed, 0x70616365);
        let fractions = (0..20)
            .map(|_| {
                if rng.next_f64() < 0.12 {
                    1.3 + 0.3 * rng.next_f64()
                } else {
                    0.85 + 0.15 * rng.next_f64()
                }
            })
            .collect();
        Self { fractions }
    }
    /// Requested power for this rider and time. Below 20% reserve, recovery is selected
    /// proportionally so the policy is continuous at the threshold and needs no hidden state.
    pub fn requested_power(
        &self,
        definition: &RiderDefinition,
        state: &RiderState,
        time: Duration,
    ) -> Result<f64> {
        if definition.id() != state.id() {
            return Err(RaceSimulationError::validation(
                C::UnknownRiderId,
                "pace plan rider mismatch",
                "id",
            ));
        }
        let Some(p) = definition.physiology() else {
            return Err(RaceSimulationError::validation(
                C::OutOfRange,
                "pack pacing requires physiology",
                "physiology",
            ));
        };
        let Some(balance) = state.w_prime_balance_j() else {
            return Err(RaceSimulationError::validation(
                C::OutOfRange,
                "pack pacing requires reserve",
                "wPrimeBalanceJ",
            ));
        };
        if balance > p.w_prime_j() {
            return Err(RaceSimulationError::validation(
                C::OutOfRange,
                "reserve exceeds capacity",
                "wPrimeBalanceJ",
            ));
        }
        let level = self.fractions[(time.as_secs() / 30 % self.fractions.len() as u64) as usize];
        let recovery_fraction =
            0.65 + (level - 0.65) * (balance / (0.2 * p.w_prime_j())).clamp(0.0, 1.0);
        Ok(p.critical_power_w() * level.min(recovery_fraction))
    }
}
