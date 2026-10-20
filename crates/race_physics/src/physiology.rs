//! Critical-power work balance and a reserve-limited sprint ceiling.
//!
//! Below CP: dB/dt = (W′ − B)(CP − P)/W′ (Skiba differential model).
//! Above CP: dB/dt = −(P − CP). The application sprint ceiling is
//! CP + (Pmax − CP) B/W′. This extra ceiling is a configurable simulation model,
//! not a claim that Skiba predicts neuromuscular fatigue.

use crate::error::{check_finite_non_negative, check_finite_positive, check_positive_duration};
use crate::PhysicsError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Fixed rider capability. CP is specified independently of FTP.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysiologyParams {
    critical_power_w: f64,
    w_prime_j: f64,
    peak_power_w: f64,
}

impl PhysiologyParams {
    /// Positive CP and W′, with finite peak power strictly above CP.
    pub fn new(
        critical_power_w: f64,
        w_prime_j: f64,
        peak_power_w: f64,
    ) -> Result<Self, PhysicsError> {
        check_finite_positive(critical_power_w, "criticalPowerW")?;
        check_finite_positive(w_prime_j, "wPrimeJ")?;
        check_finite_positive(peak_power_w, "peakPowerW")?;
        if peak_power_w <= critical_power_w {
            return Err(PhysicsError::InvalidArgument {
                name: "peakPowerW",
                value: peak_power_w.to_string(),
                message: "must exceed critical power",
            });
        }
        check_finite_positive((peak_power_w - critical_power_w) / w_prime_j, "sprintRate")?;
        Ok(Self {
            critical_power_w,
            w_prime_j,
            peak_power_w,
        })
    }
    /// Sustainable critical power, W.
    pub fn critical_power_w(&self) -> f64 {
        self.critical_power_w
    }
    /// Total supra-CP work capacity, J.
    pub fn w_prime_j(&self) -> f64 {
        self.w_prime_j
    }
    /// Fresh instantaneous sprint ceiling, W.
    pub fn peak_power_w(&self) -> f64 {
        self.peak_power_w
    }
}

impl<'de> Deserialize<'de> for PhysiologyParams {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            critical_power_w: f64,
            w_prime_j: f64,
            peak_power_w: f64,
        }
        let p = Wire::deserialize(deserializer)?;
        Self::new(p.critical_power_w, p.w_prime_j, p.peak_power_w).map_err(serde::de::Error::custom)
    }
}

/// What a physiology step actually delivered. Requested power can exceed available capability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysiologyStep {
    /// Average crank power delivered across the step, W; this alone drives motion.
    pub delivered_power_w: f64,
    /// Remaining W′, J.
    pub balance_j: f64,
}

/// Integrates a constant requested power exactly within a step, including the instant the
/// reserve-dependent sprint ceiling starts binding. Recovery uses actual delivered power.
pub fn step_physiology(
    params: &PhysiologyParams,
    balance_j: f64,
    requested_power_w: f64,
    dt: Duration,
) -> Result<PhysiologyStep, PhysicsError> {
    check_finite_non_negative(balance_j, "balanceJ")?;
    check_finite_non_negative(requested_power_w, "requestedPowerW")?;
    check_positive_duration(dt, "dt")?;
    if balance_j > params.w_prime_j {
        return Err(PhysicsError::InvalidArgument {
            name: "balanceJ",
            value: balance_j.to_string(),
            message: "must not exceed wPrimeJ",
        });
    }
    let t = dt.as_secs_f64();
    let cp = params.critical_power_w;
    let capacity = params.w_prime_j;
    if requested_power_w <= cp {
        let recovered =
            (capacity - balance_j) * -(-(cp - requested_power_w) / capacity * t).exp_m1();
        return Ok(PhysiologyStep {
            delivered_power_w: requested_power_w,
            balance_j: (balance_j + recovered).clamp(0.0, capacity),
        });
    }
    // Above CP, requested excess drains the reserve linearly until the ceiling binds.
    // Once bound: dB/dt = -k B, so B decays exponentially and cannot go negative.
    let k = (params.peak_power_w - cp) / capacity;
    let excess = requested_power_w - cp;
    let threshold = excess / k;
    let unbound_time = ((balance_j - threshold) / excess).clamp(0.0, t);
    let spent_linear = excess * unbound_time;
    let remaining = (balance_j - spent_linear).max(0.0);
    let spent_capped = remaining * -(-k * (t - unbound_time)).exp_m1();
    let spent = (spent_linear + spent_capped).min(balance_j);
    Ok(PhysiologyStep {
        delivered_power_w: cp + spent / t,
        balance_j: (balance_j - spent).clamp(0.0, capacity),
    })
}
