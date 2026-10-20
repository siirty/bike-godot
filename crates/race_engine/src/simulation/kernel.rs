use std::time::Duration;

use race_physics::{
    step_longitudinal_dynamics, step_physiology, EnvironmentParams, LongitudinalStepInput,
    PhysicsError,
};

use crate::simulation::error::{RaceSimulationError, Result, SimulationValidationCode as C};
use crate::simulation::events::detect_events;
use crate::simulation::positioning::{
    compute_drag_multipliers, resolve_lateral_slots, retain_eligible_targets, select_draft_targets,
};
use crate::simulation::validate::format_duration;
use crate::{RaceInputFrame, RaceSimulationConfig, RaceState, RaceStepResult, RiderState};

/// The deterministic stepping kernel: one validated configuration, and [`Self::step`].
///
/// Synchronous and side-effect free: no engine, trainer, I/O, timer or wall clock. The same
/// `(config, state, inputs, dt)` gives the same result bit for bit, and the headless harness and
/// the live adapter step this one object.
///
/// One step, in phases, every rider reading only the pre-step snapshot:
///
/// 1. resolve lateral slots;
/// 2. select draft targets;
/// 3. compute drag multipliers;
/// 4. one `race_physics` step per rider, with terrain sampled at that rider's pre-step distance;
/// 5. build the canonical post-step state, then retain targets under post-step geometry;
/// 6. advance time once by the fixed step;
/// 7. derive the step's events from the pre-step and post-step states.
///
/// No RNG is drawn: its state is preserved bit for bit.
#[derive(Debug, Clone)]
pub struct RaceSimulation {
    config: RaceSimulationConfig,
}

impl RaceSimulation {
    /// A simulation of `config`. Zero riders cannot be stepped.
    pub fn new(config: RaceSimulationConfig) -> Result<Self> {
        if config.riders().is_empty() {
            return Err(RaceSimulationError::validation(
                C::MissingRiderId,
                "a simulation needs at least one rider",
                "riders",
            ));
        }
        Ok(Self { config })
    }

    /// The configuration, never rewritten.
    pub fn config(&self) -> &RaceSimulationConfig {
        &self.config
    }

    /// Advances one fixed step. `dt` must equal the configured step exactly: no accumulated
    /// deltas, no substeps. The whole call is validated before anything is sampled, and a rejected
    /// call has no effect.
    pub fn step(
        &self,
        state: &RaceState,
        inputs: &RaceInputFrame,
        dt: Duration,
    ) -> Result<RaceStepResult> {
        let config = &self.config;
        if dt.is_zero() {
            return Err(RaceSimulationError::validation(
                C::NotPositive,
                format!(
                    "the step timestep must be positive, got {}",
                    format_duration(dt)
                ),
                "dt",
            ));
        }
        if dt != config.fixed_step() {
            return Err(RaceSimulationError::validation(
                C::TimestepMismatch,
                format!(
                    "the step timestep ({}) must equal the configured fixed step ({}); variable \
                     steps and accumulated deltas are not accepted",
                    format_duration(dt),
                    format_duration(config.fixed_step())
                ),
                "dt",
            ));
        }
        config.validate_state(state, "state")?;
        config.validate_input_frame(inputs, "inputs")?;

        let slots = resolve_lateral_slots(state, inputs);
        let targets = select_draft_targets(config, state, &slots, inputs);
        let drag = compute_drag_multipliers(config, state, &slots, &targets);

        let mut next = Vec::with_capacity(state.riders().len());
        for (i, rider) in state.riders().iter().enumerate() {
            let input = inputs.input_for(rider.id()).expect("validated total frame");
            let definition = config
                .definition_for(rider.id())
                .expect("validated rider set");
            let environment = EnvironmentParams {
                grade_fraction: config.terrain().grade_at(rider.distance_m()),
                air_density_kg_m3: config.air_density_kg_m3(),
                headwind_ms: config.headwind_ms(),
            };
            let physiology = definition
                .physiology()
                .map(|p| {
                    step_physiology(
                        p,
                        rider.w_prime_balance_j().expect("validated reserve"),
                        input.power_watts(),
                        config.fixed_step(),
                    )
                })
                .transpose()
                .map_err(|e| physics_error(e, i))?;
            let dynamics = step_longitudinal_dynamics(
                &LongitudinalStepInput {
                    speed_ms: rider.speed_ms(),
                    power_watts: physiology.map_or(input.power_watts(), |p| p.delivered_power_w),
                    rider: *definition.physical(),
                    environment,
                    timestep: config.fixed_step(),
                    braking_force_n: input.braking_force_n(),
                    aerodynamic_drag_multiplier: drag[i],
                },
                config.dynamics(),
            )
            .map_err(|e| physics_error(e, i))?;
            let mut stepped = RiderState::stepped(
                rider.id().clone(),
                rider.distance_m() + dynamics.distance_delta_m,
                dynamics.next_speed_ms,
                slots[i],
                None,
            )?;
            if let Some(p) = physiology {
                stepped = stepped.with_w_prime_balance(p.balance_j)?;
            }
            next.push(stepped);
        }

        let time = state.simulation_time() + config.fixed_step();
        let post_step = RaceState::canonical(time, state.random_state(), next)?;
        let retained = retain_eligible_targets(config, &post_step, &slots, &targets);
        let riders = post_step
            .riders()
            .iter()
            .zip(retained)
            .map(|(r, target)| {
                let stepped = RiderState::stepped(
                    r.id().clone(),
                    r.distance_m(),
                    r.speed_ms(),
                    r.lateral_slot(),
                    target,
                )?;
                match r.w_prime_balance_j() {
                    Some(b) => stepped.with_w_prime_balance(b),
                    None => Ok(stepped),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let next_state = RaceState::canonical(time, state.random_state(), riders)?;
        let events = detect_events(config, state, &next_state)?;
        Ok(RaceStepResult { next_state, events })
    }
}

/// The physics step re-validates what the contracts already guarantee, except one bound the
/// contracts are looser on (drivetrain efficiency above 1) and non-finite results from
/// absurd-but-legal parameters. Both become typed failures on the rider instead of escaping.
fn physics_error(e: PhysicsError, rider_index: usize) -> RaceSimulationError {
    let (code, field) = match &e {
        PhysicsError::InvalidArgument { name, .. } => (C::OutOfRange, *name),
        PhysicsError::NonFiniteResult { name, .. } => (C::NotFinite, *name),
        PhysicsError::Solver(_) => (C::NotFinite, "dynamics"),
    };
    RaceSimulationError::validation(
        code,
        e.to_string(),
        format!("state.riders[{rider_index}].{field}"),
    )
}
