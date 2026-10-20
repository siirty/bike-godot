//! The closed-form power for a speed or grade, and its two safeguarded inverses.
//!
//! Both inverses are Newton–Raphson with a bisection fallback inside a fixed bracket. `P(v)` and
//! `P(grade)` are monotonic for physically sane inputs, so a solution inside the bracket is unique
//! when one exists. Operation order is stable and covered by frozen regression vectors.

use crate::forces::{aero_drag_force_n, gravity_force_n, rolling_resistance_force_n};
use crate::{EnvironmentParams, PhysicsError, RiderPhysicalParams};

/// Lower end of the speed bracket, in m/s. There is no "braking via wattage".
pub const MIN_SOLVABLE_SPEED_MS: f64 = 0.0;
/// Upper end of the speed bracket, in m/s (108 km/h).
pub const MAX_SOLVABLE_SPEED_MS: f64 = 30.0;
/// Lower end of the grade bracket (a 12 % descent).
pub const MIN_SOLVABLE_GRADE: f64 = -0.12;
/// Upper end of the grade bracket (a 15 % climb).
pub const MAX_SOLVABLE_GRADE: f64 = 0.15;

const DERIVATIVE_STEP_MS: f64 = 1e-4;

/// Convergence controls shared by both solvers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolverOptions {
    /// Stop when the power residual is below this, in watts.
    pub tolerance_watts: f64,
    /// Give up after this many iterations.
    pub max_iterations: u32,
}

impl Default for SolverOptions {
    fn default() -> Self {
        Self {
            tolerance_watts: 0.01,
            max_iterations: 100,
        }
    }
}

/// Power (W) needed to hold `speed_ms`:
/// `[m·g·(sinθ + Crr·cosθ) + 0.5·ρ·CdA·|v+w|·(v+w)]·v/η`.
pub fn power_for_speed(speed_ms: f64, rider: &RiderPhysicalParams, env: &EnvironmentParams) -> f64 {
    let total_force_n = gravity_force_n(rider.total_mass_kg, env.grade_fraction)
        + rolling_resistance_force_n(rider.total_mass_kg, env.grade_fraction, rider.crr)
        + aero_drag_force_n(
            env.air_density_kg_m3,
            rider.cda_m2,
            speed_ms,
            env.headwind_ms,
        );
    total_force_n * speed_ms / rider.drive_train_efficiency
}

/// [`power_for_speed`] with `env`'s grade replaced by `grade_fraction`.
pub fn power_for_grade(
    speed_ms: f64,
    grade_fraction: f64,
    rider: &RiderPhysicalParams,
    env: &EnvironmentParams,
) -> f64 {
    power_for_speed(
        speed_ms,
        rider,
        &EnvironmentParams {
            grade_fraction,
            ..*env
        },
    )
}

/// The speed (m/s) at which `rider` sustains `power_watts` in `env`.
///
/// Zero power returns zero. Fails with [`PhysicsError::InvalidArgument`] for negative power, and
/// with [`PhysicsError::Solver`] when the power needs more than [`MAX_SOLVABLE_SPEED_MS`] or the
/// iteration does not converge.
pub fn solve_speed_for_power(
    power_watts: f64,
    rider: &RiderPhysicalParams,
    env: &EnvironmentParams,
    options: SolverOptions,
) -> Result<f64, PhysicsError> {
    if power_watts < 0.0 {
        return Err(PhysicsError::InvalidArgument {
            name: "powerWatts",
            value: power_watts.to_string(),
            message: "must be non-negative",
        });
    }
    if power_watts == 0.0 {
        return Ok(0.0);
    }

    let mut lo = MIN_SOLVABLE_SPEED_MS;
    let mut hi = MAX_SOLVABLE_SPEED_MS;
    let residual_at_hi = power_for_speed(hi, rider, env) - power_watts;
    if residual_at_hi < 0.0 {
        return Err(PhysicsError::Solver(format!(
            "no solution within v ∈ [{MIN_SOLVABLE_SPEED_MS}, {MAX_SOLVABLE_SPEED_MS}] m/s for \
             powerWatts={power_watts} (P({MAX_SOLVABLE_SPEED_MS} m/s)={}W is still less than the \
             requested power)",
            residual_at_hi + power_watts
        )));
    }

    let mut v = (lo + hi) / 2.0;
    for _ in 0..options.max_iterations {
        let residual = power_for_speed(v, rider, env) - power_watts;
        if residual.abs() < options.tolerance_watts {
            return Ok(v);
        }
        if residual < 0.0 {
            lo = v;
        } else {
            hi = v;
        }
        let derivative = numerical_derivative(v, rider, env);
        let newton_step = if derivative.abs() > 1e-9 {
            v - residual / derivative
        } else {
            f64::NAN
        };
        v = if newton_step.is_nan() || newton_step <= lo || newton_step >= hi {
            (lo + hi) / 2.0
        } else {
            newton_step
        };
    }
    Err(PhysicsError::Solver(format!(
        "solveSpeedForPower did not converge within {} iterations for powerWatts={power_watts}",
        options.max_iterations
    )))
}

/// The grade (rise/run) on which `rider` sustains `power_watts` at `speed_ms`, with `env`'s other
/// quantities. The terrain-synthesis primitive: the result is frozen into a course, never adapted
/// live.
///
/// Fails with [`PhysicsError::InvalidArgument`] for negative power or speed, and with
/// [`PhysicsError::Solver`] when no grade in `[MIN_SOLVABLE_GRADE, MAX_SOLVABLE_GRADE]` works or
/// the iteration does not converge.
pub fn solve_grade_for_power(
    power_watts: f64,
    speed_ms: f64,
    rider: &RiderPhysicalParams,
    env: &EnvironmentParams,
    options: SolverOptions,
) -> Result<f64, PhysicsError> {
    if power_watts < 0.0 {
        return Err(PhysicsError::InvalidArgument {
            name: "powerWatts",
            value: power_watts.to_string(),
            message: "must be non-negative",
        });
    }
    if speed_ms < 0.0 {
        return Err(PhysicsError::InvalidArgument {
            name: "speedMs",
            value: speed_ms.to_string(),
            message: "must be non-negative",
        });
    }
    let power_at = |grade: f64| power_for_grade(speed_ms, grade, rider, env);

    let residual_at_min = power_at(MIN_SOLVABLE_GRADE) - power_watts;
    let residual_at_max = power_at(MAX_SOLVABLE_GRADE) - power_watts;
    if residual_at_min > 0.0 {
        return Err(PhysicsError::Solver(format!(
            "no solution within grade ∈ [{MIN_SOLVABLE_GRADE}, {MAX_SOLVABLE_GRADE}] for \
             powerWatts={power_watts} at speedMs={speed_ms} (even the steepest descent P={}W \
             exceeds target)",
            residual_at_min + power_watts
        )));
    }
    if residual_at_max < 0.0 {
        return Err(PhysicsError::Solver(format!(
            "no solution within grade ∈ [{MIN_SOLVABLE_GRADE}, {MAX_SOLVABLE_GRADE}] for \
             powerWatts={power_watts} at speedMs={speed_ms} (steepest climb P={}W falls short of \
             target)",
            residual_at_max + power_watts
        )));
    }

    let mut lo = MIN_SOLVABLE_GRADE;
    let mut hi = MAX_SOLVABLE_GRADE;
    let mut g = (lo + hi) / 2.0;
    for _ in 0..options.max_iterations {
        let residual = power_at(g) - power_watts;
        if residual.abs() < options.tolerance_watts {
            return Ok(g);
        }
        if residual < 0.0 {
            lo = g;
        } else {
            hi = g;
        }
        let residual_lo = power_at(lo) - power_watts;
        let residual_hi = power_at(hi) - power_watts;
        let derivative = (residual_hi - residual_lo) / (hi - lo);
        let newton_step = if derivative.abs() > 1e-9 {
            g - residual / derivative
        } else {
            f64::NAN
        };
        g = if newton_step.is_nan() || newton_step <= lo || newton_step >= hi {
            (lo + hi) / 2.0
        } else {
            newton_step
        };
    }
    Err(PhysicsError::Solver(format!(
        "solveGradeForPower did not converge within {} iterations for powerWatts={power_watts} \
         at speedMs={speed_ms}",
        options.max_iterations
    )))
}

fn numerical_derivative(v: f64, rider: &RiderPhysicalParams, env: &EnvironmentParams) -> f64 {
    let v_plus = clamp_speed(v + DERIVATIVE_STEP_MS);
    let v_minus = clamp_speed(v - DERIVATIVE_STEP_MS);
    let denom = v_plus - v_minus;
    if denom == 0.0 {
        return 0.0;
    }
    (power_for_speed(v_plus, rider, env) - power_for_speed(v_minus, rider, env)) / denom
}

fn clamp_speed(speed_ms: f64) -> f64 {
    // NaN passes through, as in the prototype's comparison chain.
    speed_ms.clamp(MIN_SOLVABLE_SPEED_MS, MAX_SOLVABLE_SPEED_MS)
}
