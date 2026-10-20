//! Pure cycling physics: the gravity/rolling/aero forces, the power↔speed and power↔grade
//! solvers, and the one fixed-step longitudinal integrator every simulated rider is advanced with.
//!
//! A parity-tested port of the Dart prototype's `race_physics` package. No I/O, no wall clock, no
//! engine dependency. Drafting enters only as an aerodynamic-drag multiplier (see
//! [step_longitudinal_dynamics]); nothing here can scale power, mass, rolling resistance, grade or
//! speed to influence an outcome.
//!
//! Every public entry point validates its whole input first and reports the caller-facing field
//! name in a [PhysicsError::InvalidArgument]. `race_engine` owns typed simulation validation.

mod defaults;
mod dynamics;
mod error;
mod forces;
mod model;
mod solver;
mod timestep;

#[cfg(feature = "testing")]
pub mod convergence;

pub use defaults::PhysicsDefaults;
pub use dynamics::{
    step_longitudinal_dynamics, LongitudinalDynamicsConfig, LongitudinalStepInput,
    LongitudinalStepResult, DEFAULT_MAX_PROPULSIVE_FORCE_N,
};
pub use error::PhysicsError;
pub use forces::{
    aero_drag_force_n, gravity_force_n, rolling_resistance_force_n, GRAVITY_ACCELERATION_MS2,
};
pub use model::{EnvironmentParams, RiderPhysicalParams};
pub use solver::{
    power_for_grade, power_for_speed, solve_grade_for_power, solve_speed_for_power, SolverOptions,
    MAX_SOLVABLE_GRADE, MAX_SOLVABLE_SPEED_MS, MIN_SOLVABLE_GRADE, MIN_SOLVABLE_SPEED_MS,
};
pub use timestep::SimulationTimestep;
