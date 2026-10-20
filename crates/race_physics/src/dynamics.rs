//! The fixed-step longitudinal integrator.

use std::time::Duration;

use crate::error::{
    check_finite, check_finite_non_negative, check_finite_positive, check_finite_unit_fraction,
    check_positive_duration,
};
use crate::forces::{aero_drag_force_n, gravity_force_n, rolling_resistance_force_n};
use crate::{EnvironmentParams, PhysicsError, RiderPhysicalParams};

/// The default low-speed tractive-force cap, in newtons: about the peak force a strong sprinter
/// puts through the rear wheel. It binds only below `wheelPower / 500 N` (0.6 m/s at 300 W). It is
/// deliberately not a gearing or torque model; when one arrives it replaces this cap.
pub const DEFAULT_MAX_PROPULSIVE_FORCE_N: f64 = 500.0;

/// The numerical (not physical) parameters of the integrator. It travels explicitly, in
/// `race_engine`'s simulation config and therefore its snapshots, so a release cannot silently
/// change how a saved race resumes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongitudinalDynamicsConfig {
    max_propulsive_force_n: f64,
}

impl Default for LongitudinalDynamicsConfig {
    fn default() -> Self {
        Self {
            max_propulsive_force_n: DEFAULT_MAX_PROPULSIVE_FORCE_N,
        }
    }
}

impl LongitudinalDynamicsConfig {
    /// A config with the given propulsive-force cap, which must be finite and positive.
    pub fn new(max_propulsive_force_n: f64) -> Result<Self, PhysicsError> {
        check_finite_positive(max_propulsive_force_n, "maxPropulsiveForceN")?;
        Ok(Self {
            max_propulsive_force_n,
        })
    }

    /// The most propulsive force rider power may produce, in newtons.
    pub fn max_propulsive_force_n(&self) -> f64 {
        self.max_propulsive_force_n
    }
}

/// Everything one rider's step depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongitudinalStepInput {
    /// Pre-step speed, in m/s. Finite, non-negative.
    pub speed_ms: f64,
    /// Rider (crank) power, in watts. Finite, non-negative.
    pub power_watts: f64,
    /// The rider's fixed physical parameters.
    pub rider: RiderPhysicalParams,
    /// Grade, air and wind for this step.
    pub environment: EnvironmentParams,
    /// The step length. Positive.
    pub timestep: Duration,
    /// Braking force, in newtons. Finite, non-negative.
    pub braking_force_n: f64,
    /// Drafting shelter: `1` is clean air. In `(0, 1]`. Scales the aero term only.
    pub aerodynamic_drag_multiplier: f64,
}

impl LongitudinalStepInput {
    /// A step with no braking, in clean air.
    pub fn new(
        speed_ms: f64,
        power_watts: f64,
        rider: RiderPhysicalParams,
        environment: EnvironmentParams,
        timestep: Duration,
    ) -> Self {
        Self {
            speed_ms,
            power_watts,
            rider,
            environment,
            timestep,
            braking_force_n: 0.0,
            aerodynamic_drag_multiplier: 1.0,
        }
    }
}

/// One integrated step: the new kinematic state plus the whole force balance that produced it, so
/// tests and debugging never re-derive the forces. Every field is finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongitudinalStepResult {
    /// Speed at the end of the step, in m/s. Never negative.
    pub next_speed_ms: f64,
    /// Distance covered during the step, in metres. Never negative.
    pub distance_delta_m: f64,
    /// Forward force from rider power, in newtons. Bounded by the configured cap.
    pub propulsive_force_n: f64,
    /// Slope component of gravity, in newtons. Positive resists.
    pub gravity_force_n: f64,
    /// Rolling resistance, in newtons.
    pub rolling_resistance_force_n: f64,
    /// Aero drag before the drafting multiplier, in newtons.
    pub raw_aero_drag_force_n: f64,
    /// Aero drag after the drafting multiplier: the value in [`Self::net_force_n`].
    pub applied_aero_drag_force_n: f64,
    /// Braking force applied, in newtons.
    pub braking_force_n: f64,
    /// `propulsive − gravity − rolling − appliedAero − braking`, in newtons.
    pub net_force_n: f64,
    /// Net force over mass, in m/s². Force-derived: not what was applied if the clamp hit.
    pub raw_acceleration_ms2: f64,
    /// The unconstrained next speed was negative and was clamped to zero: the rider stopped
    /// inside the step.
    pub zero_speed_clamp_applied: bool,
}

/// Advances one rider one fixed timestep with a single semi-implicit Euler step.
///
/// 1. Every force is computed from the pre-step speed.
/// 2. The unconstrained next speed is `speed + rawAccel·dt`.
/// 3. It is clamped to `>= 0`: this model never rides backwards.
/// 4. Distance advances by the clamped post-step speed times `dt`.
///
/// Propulsive force is `min(wheelPower / speed, cap)`, evaluated as the cap at and below the
/// crossover speed so a standstill needs no special case. Zero power is zero force.
///
/// There are no substeps and no adaptive refinement. A race's trajectory depends on its inputs,
/// not on how hard the arithmetic was. A finer step is a measuring stick ([`crate::convergence`]),
/// never a refinement of this one.
///
/// The drafting multiplier scales the aerodynamic term and nothing else. That is the difference
/// between a drafting model and a rubber-banding lever.
///
/// Every input is validated before anything is computed. Negative grade and headwind are valid
/// (a descent and a tailwind).
pub fn step_longitudinal_dynamics(
    input: &LongitudinalStepInput,
    config: &LongitudinalDynamicsConfig,
) -> Result<LongitudinalStepResult, PhysicsError> {
    let LongitudinalStepInput {
        speed_ms,
        power_watts,
        rider,
        environment,
        timestep,
        braking_force_n,
        aerodynamic_drag_multiplier,
    } = *input;
    check_finite_non_negative(speed_ms, "speedMs")?;
    check_finite_non_negative(power_watts, "powerWatts")?;
    check_finite_non_negative(braking_force_n, "brakingForceN")?;
    check_finite_unit_fraction(aerodynamic_drag_multiplier, "aerodynamicDragMultiplier")?;
    check_positive_duration(timestep, "timestep")?;
    check_finite_positive(rider.total_mass_kg, "rider.totalMassKg")?;
    check_finite_non_negative(rider.crr, "rider.crr")?;
    check_finite_non_negative(rider.cda_m2, "rider.cdaM2")?;
    check_finite_unit_fraction(rider.drive_train_efficiency, "rider.driveTrainEfficiency")?;
    check_finite(environment.grade_fraction, "environment.gradeFraction")?;
    check_finite_positive(environment.air_density_kg_m3, "environment.airDensityKgM3")?;
    check_finite(environment.headwind_ms, "environment.headwindMs")?;

    let propulsive_force_n = propulsive_force_n(
        speed_ms,
        power_watts,
        rider.drive_train_efficiency,
        config.max_propulsive_force_n,
    );
    let gravity_n = gravity_force_n(rider.total_mass_kg, environment.grade_fraction);
    let rolling_n =
        rolling_resistance_force_n(rider.total_mass_kg, environment.grade_fraction, rider.crr);
    let raw_aero_n = aero_drag_force_n(
        environment.air_density_kg_m3,
        rider.cda_m2,
        speed_ms,
        environment.headwind_ms,
    );
    let applied_aero_n = aerodynamic_drag_multiplier * raw_aero_n;

    let net_force_n = propulsive_force_n - gravity_n - rolling_n - applied_aero_n - braking_force_n;
    let raw_acceleration_ms2 = net_force_n / rider.total_mass_kg;

    let timestep_seconds = timestep.as_micros() as f64 / 1_000_000.0;
    let unconstrained_next_speed_ms = speed_ms + raw_acceleration_ms2 * timestep_seconds;
    let clamped = unconstrained_next_speed_ms < 0.0;
    let next_speed_ms = if clamped {
        0.0
    } else {
        unconstrained_next_speed_ms
    };

    let result = LongitudinalStepResult {
        next_speed_ms,
        distance_delta_m: next_speed_ms * timestep_seconds,
        propulsive_force_n,
        gravity_force_n: gravity_n,
        rolling_resistance_force_n: rolling_n,
        raw_aero_drag_force_n: raw_aero_n,
        applied_aero_drag_force_n: applied_aero_n,
        braking_force_n,
        net_force_n,
        raw_acceleration_ms2,
        zero_speed_clamp_applied: clamped,
    };
    for (name, value) in [
        ("nextSpeedMs", result.next_speed_ms),
        ("distanceDeltaM", result.distance_delta_m),
        ("propulsiveForceN", result.propulsive_force_n),
        ("gravityForceN", result.gravity_force_n),
        ("rollingResistanceForceN", result.rolling_resistance_force_n),
        ("rawAeroDragForceN", result.raw_aero_drag_force_n),
        ("appliedAeroDragForceN", result.applied_aero_drag_force_n),
        ("brakingForceN", result.braking_force_n),
        ("netForceN", result.net_force_n),
        ("rawAccelerationMs2", result.raw_acceleration_ms2),
    ] {
        if !value.is_finite() {
            return Err(PhysicsError::NonFiniteResult { name, value });
        }
    }
    Ok(result)
}

/// `min(wheelPower / speed, cap)`, written so a standstill is arithmetic rather than a guard.
fn propulsive_force_n(
    speed_ms: f64,
    power_watts: f64,
    drive_train_efficiency: f64,
    max_propulsive_force_n: f64,
) -> f64 {
    if power_watts == 0.0 {
        return 0.0;
    }
    let wheel_power_w = power_watts * drive_train_efficiency;
    let crossover_speed_ms = wheel_power_w / max_propulsive_force_n;
    if speed_ms <= crossover_speed_ms {
        return max_propulsive_force_n;
    }
    wheel_power_w / speed_ms
}
