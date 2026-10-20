use std::time::Duration;

/// The fixed simulation step the longitudinal dynamics were validated at, and the error budget it
/// buys. Measured by the convergence corpus (`--features testing`, [`crate::convergence`]); the
/// table is in this crate's README, and `tests/convergence.rs` fails if a budget stops holding.
pub struct SimulationTimestep;

impl SimulationTimestep {
    /// The fixed step races run at: 50 ms (20 Hz).
    pub const RECOMMENDED: Duration = Duration::from_millis(50);
    /// Worst-case speed error at [`Self::RECOMMENDED`], in m/s (hard acceleration from rest).
    pub const SPEED_ERROR_BUDGET_MS: f64 = 0.10;
    /// Worst-case cumulative distance error, in metres. An offset from transients, not a drift.
    pub const DISTANCE_ERROR_BUDGET_M: f64 = 1.0;
    /// Worst-case error in the gap between two riders on near-matching trajectories, in metres.
    /// The budget drafting thresholds are designed against.
    pub const RIDER_GAP_ERROR_BUDGET_M: f64 = 0.01;
    /// Worst-case error in when a braking rider stops: one step, quantized by construction.
    pub const STOPPING_TIME_ERROR_BUDGET: Duration = Self::RECOMMENDED;
    /// Worst-case error in how far a braking rider travels before stopping, in metres.
    pub const STOPPING_DISTANCE_ERROR_BUDGET_M: f64 = 0.5;
}
