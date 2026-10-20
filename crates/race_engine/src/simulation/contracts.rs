//! The validated simulation contracts. Each type validates on its only construction path, so an
//! illegal value is unrepresentable rather than merely discouraged.

use std::collections::HashSet;
use std::time::Duration;

use race_physics::{LongitudinalDynamicsConfig, PhysicsDefaults, RiderPhysicalParams};

use crate::dart_json::dart_double;
use crate::simulation::error::{at, RaceSimulationError, Result, SimulationValidationCode as C};
use crate::simulation::validate::{
    check_canonical, check_finite, check_finite_non_negative, check_finite_positive,
    check_non_negative_int, check_positive_duration, check_positive_int,
};
use crate::{RaceEvent, RandomState, RiderId, SimulationTerrain};

/// Everything about a rider that does not change while the simulation runs. Capability lives
/// here, in the configuration the kernel never rewrites, which is the mechanical form of the
/// no-rubber-banding rule. Physiology (CP/W′, sprint) is later-slice work.
#[derive(Debug, Clone, PartialEq)]
pub struct RiderDefinition {
    id: RiderId,
    physical: RiderPhysicalParams,
}

impl RiderDefinition {
    /// A validated definition: mass and efficiency positive, Crr and CdA non-negative, all finite.
    pub fn new(id: RiderId, physical: RiderPhysicalParams) -> Result<Self> {
        at("physical", || {
            check_finite_positive(physical.total_mass_kg, "totalMassKg")?;
            check_finite_non_negative(physical.crr, "crr")?;
            check_finite_non_negative(physical.cda_m2, "cdaM2")?;
            check_finite_positive(physical.drive_train_efficiency, "driveTrainEfficiency")?;
            Ok(())
        })?;
        Ok(Self { id, physical })
    }

    /// The rider's identity.
    pub fn id(&self) -> &RiderId {
        &self.id
    }

    /// `race_physics`' own model, fixed for the session.
    pub fn physical(&self) -> &RiderPhysicalParams {
        &self.physical
    }
}

/// The minimum 1.5-D drafting model. Shelter is aerodynamic drag only.
///
/// For an eligible target at pre-step gap `g` and slot separation `s`:
/// `m = near[s] + (1 − near[s]) · g / maxGap`, clamped to `[near[s], 1]`. Eligible means
/// `0 < g < maxGap` and `s < near.len()`; otherwise the multiplier is exactly 1. The constants are
/// explicit and uncalibrated by design; there is no default model.
#[derive(Debug, Clone, PartialEq)]
pub struct BasicDraftingConfig {
    max_gap_m: f64,
    minimum_drag_multiplier_by_slot_offset: Vec<f64>,
}

impl BasicDraftingConfig {
    /// Validates: positive gap; a non-empty, non-decreasing list of values in `(0, 1]` whose
    /// first entry is `< 1`.
    pub fn new(max_gap_m: f64, minimum_drag_multiplier_by_slot_offset: Vec<f64>) -> Result<Self> {
        check_finite_positive(max_gap_m, "maxGapM")?;
        let list = &minimum_drag_multiplier_by_slot_offset;
        if list.is_empty() {
            return Err(RaceSimulationError::validation(
                C::DraftingSlotsEmpty,
                "minimumDragMultiplierBySlotOffset needs at least one value (index 0 is the \
                 same-slot shelter)",
                "minimumDragMultiplierBySlotOffset",
            ));
        }
        for (i, &value) in list.iter().enumerate() {
            if !value.is_finite() || value <= 0.0 || value > 1.0 {
                return Err(RaceSimulationError::validation(
                    C::DraftingMultiplierOutOfRange,
                    format!(
                        "drag multipliers must be finite and in (0, 1], got {} at offset {i}",
                        dart_double(value)
                    ),
                    format!("minimumDragMultiplierBySlotOffset[{i}]"),
                ));
            }
            if i > 0 && value < list[i - 1] {
                return Err(RaceSimulationError::validation(
                    C::DraftingMultipliersNotMonotonic,
                    format!(
                        "drag multipliers must be monotonically non-decreasing so moving farther \
                         laterally can never improve shelter: {} at offset {i} does not follow {}",
                        dart_double(value),
                        dart_double(list[i - 1])
                    ),
                    format!("minimumDragMultiplierBySlotOffset[{i}]"),
                ));
            }
        }
        if list[0] >= 1.0 {
            return Err(RaceSimulationError::validation(
                C::DraftingSameSlotNotSheltering,
                "the offset-0 multiplier must be < 1 so the model provides an observable \
                 same-slot benefit",
                "minimumDragMultiplierBySlotOffset[0]",
            ));
        }
        Ok(Self {
            max_gap_m,
            minimum_drag_multiplier_by_slot_offset,
        })
    }

    /// The longest gap at which a rider may still draft, in metres.
    pub fn max_gap_m(&self) -> f64 {
        self.max_gap_m
    }

    /// Best shelter per absolute lateral-slot separation.
    pub fn minimum_drag_multiplier_by_slot_offset(&self) -> &[f64] {
        &self.minimum_drag_multiplier_by_slot_offset
    }

    /// Offsets `[0, slot_count)` are eligible.
    pub fn slot_count(&self) -> usize {
        self.minimum_drag_multiplier_by_slot_offset.len()
    }

    pub(crate) fn multiplier_for(&self, offset: usize) -> f64 {
        self.minimum_drag_multiplier_by_slot_offset[offset]
    }
}

/// Everything about a simulation that does not change while it runs. The kernel never rewrites
/// it. Self-contained: terrain is a value and riders are definitions, so a decoded snapshot can be
/// stepped without anything that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceSimulationConfig {
    fixed_step: Duration,
    riders: Vec<RiderDefinition>,
    terrain: SimulationTerrain,
    dynamics: LongitudinalDynamicsConfig,
    drafting: Option<BasicDraftingConfig>,
    lateral_slot_count: usize,
    air_density_kg_m3: f64,
    headwind_ms: f64,
}

/// Inputs for [`RaceSimulationConfig::new`]. `fixed_step`, `dynamics` and `drafting` have no
/// defaults on purpose: each can change a physics result and must be chosen, then travel in every
/// snapshot.
#[derive(Debug, Clone)]
pub struct RaceSimulationConfigSpec {
    /// The step. Pass `SimulationTimestep::RECOMMENDED` unless measured otherwise.
    pub fixed_step: Duration,
    /// The field, in any order.
    pub riders: Vec<RiderDefinition>,
    /// The course.
    pub terrain: SimulationTerrain,
    /// Integrator parameters.
    pub dynamics: LongitudinalDynamicsConfig,
    /// The drafting model, or `None` for a modelled absence.
    pub drafting: Option<BasicDraftingConfig>,
    /// Discrete lateral positions. At least 1.
    pub lateral_slot_count: i64,
    /// Ambient air density, kg/m³.
    pub air_density_kg_m3: f64,
    /// Ambient headwind, m/s.
    pub headwind_ms: f64,
}

impl RaceSimulationConfigSpec {
    /// A spec with one lateral slot, sea-level air and no wind.
    pub fn new(
        fixed_step: Duration,
        riders: Vec<RiderDefinition>,
        terrain: SimulationTerrain,
        dynamics: LongitudinalDynamicsConfig,
        drafting: Option<BasicDraftingConfig>,
    ) -> Self {
        Self {
            fixed_step,
            riders,
            terrain,
            dynamics,
            drafting,
            lateral_slot_count: 1,
            air_density_kg_m3: PhysicsDefaults::AIR_DENSITY_KG_M3,
            headwind_ms: 0.0,
        }
    }
}

impl RaceSimulationConfig {
    /// Canonicalises riders into ascending ID order and validates every scalar bound. The same
    /// set from differently ordered sources gives an identical config and identical snapshot bytes.
    pub fn new(spec: RaceSimulationConfigSpec) -> Result<Self> {
        let mut riders = spec.riders;
        riders.sort_by(|a, b| a.id.cmp(&b.id));
        Self::canonical(RaceSimulationConfigSpec { riders, ..spec })
    }

    /// Adopts riders that must already be canonical; a non-canonical list is
    /// [`C::RidersNotCanonical`].
    pub fn canonical(spec: RaceSimulationConfigSpec) -> Result<Self> {
        check_positive_duration(spec.fixed_step, "fixedStep")?;
        check_positive_int(spec.lateral_slot_count, "lateralSlotCount")?;
        check_finite_positive(spec.air_density_kg_m3, "airDensityKgM3")?;
        check_finite(spec.headwind_ms, "headwindMs")?;
        check_canonical(&spec.riders, |r| &r.id, "riders", ".id")?;
        Ok(Self {
            fixed_step: spec.fixed_step,
            riders: spec.riders,
            terrain: spec.terrain,
            dynamics: spec.dynamics,
            drafting: spec.drafting,
            lateral_slot_count: spec.lateral_slot_count as usize,
            air_density_kg_m3: spec.air_density_kg_m3,
            headwind_ms: spec.headwind_ms,
        })
    }

    /// The fixed step.
    pub fn fixed_step(&self) -> Duration {
        self.fixed_step
    }
    /// The field, canonical.
    pub fn riders(&self) -> &[RiderDefinition] {
        &self.riders
    }
    /// The course.
    pub fn terrain(&self) -> &SimulationTerrain {
        &self.terrain
    }
    /// Integrator parameters.
    pub fn dynamics(&self) -> &LongitudinalDynamicsConfig {
        &self.dynamics
    }
    /// The drafting model, if any.
    pub fn drafting(&self) -> Option<&BasicDraftingConfig> {
        self.drafting.as_ref()
    }
    /// Lateral positions.
    pub fn lateral_slot_count(&self) -> usize {
        self.lateral_slot_count
    }
    /// Ambient air density.
    pub fn air_density_kg_m3(&self) -> f64 {
        self.air_density_kg_m3
    }
    /// Ambient headwind.
    pub fn headwind_ms(&self) -> f64 {
        self.headwind_ms
    }

    /// Configured rider IDs, canonical.
    pub fn rider_ids(&self) -> Vec<&RiderId> {
        self.riders.iter().map(|r| &r.id).collect()
    }

    /// The definition of `id`.
    pub fn definition_for(&self, id: &RiderId) -> Option<&RiderDefinition> {
        self.riders
            .binary_search_by(|r| r.id.cmp(id))
            .ok()
            .map(|i| &self.riders[i])
    }

    /// Fails unless `state` is a legal state of this configuration: exact rider set, slots below
    /// the count, draft targets configured. Reported under `path`.
    pub fn validate_state(&self, state: &RaceState, path: &str) -> Result<()> {
        at(path, || {
            self.check_rider_set(
                &state.riders.iter().map(|r| &r.id).collect::<Vec<_>>(),
                "riders",
            )?;
            for (i, rider) in state.riders.iter().enumerate() {
                at(&format!("riders[{i}]"), || {
                    self.check_lateral_slot(rider.lateral_slot, "lateralSlot")?;
                    self.check_draft_target(rider.draft_target_id.as_ref(), "draftTargetId")
                })?;
            }
            Ok(())
        })
    }

    /// Fails unless `frame` is a legal input frame for this configuration.
    pub fn validate_input_frame(&self, frame: &RaceInputFrame, path: &str) -> Result<()> {
        at(path, || {
            self.check_rider_set(
                &frame.inputs.iter().map(|i| &i.id).collect::<Vec<_>>(),
                "inputs",
            )?;
            for (i, input) in frame.inputs.iter().enumerate() {
                at(&format!("inputs[{i}]"), || {
                    if let Some(slot) = input.lateral_slot {
                        self.check_lateral_slot(slot, "lateralSlot")?;
                    }
                    self.check_draft_target(input.draft_target_id.as_ref(), "draftTargetId")
                })?;
            }
            Ok(())
        })
    }

    fn check_rider_set(&self, ids: &[&RiderId], path: &str) -> Result<()> {
        let supplied: HashSet<&RiderId> = ids.iter().copied().collect();
        for r in &self.riders {
            if !supplied.contains(&r.id) {
                return Err(RaceSimulationError::validation(
                    C::MissingRiderId,
                    format!("configured rider \"{}\" has no entry", r.id),
                    path,
                ));
            }
        }
        for (i, id) in ids.iter().enumerate() {
            if self.definition_for(id).is_none() {
                return Err(RaceSimulationError::validation(
                    C::UnknownRiderId,
                    format!("\"{id}\" is not a configured rider"),
                    format!("{path}[{i}]"),
                ));
            }
        }
        Ok(())
    }

    fn check_lateral_slot(&self, slot: usize, path: &str) -> Result<()> {
        if slot >= self.lateral_slot_count {
            return Err(RaceSimulationError::validation(
                C::OutOfRange,
                format!(
                    "lateral slot {slot} is not below the configured lateralSlotCount ({})",
                    self.lateral_slot_count
                ),
                path,
            ));
        }
        Ok(())
    }

    fn check_draft_target(&self, target: Option<&RiderId>, path: &str) -> Result<()> {
        if let Some(target) = target {
            if self.definition_for(target).is_none() {
                return Err(RaceSimulationError::validation(
                    C::UnknownRiderId,
                    format!("draft target \"{target}\" is not a configured rider"),
                    path,
                ));
            }
        }
        Ok(())
    }
}

/// One rider's dynamic state: only what a step can change.
#[derive(Debug, Clone, PartialEq)]
pub struct RiderState {
    id: RiderId,
    distance_m: f64,
    speed_ms: f64,
    lateral_slot: usize,
    draft_target_id: Option<RiderId>,
}

impl RiderState {
    /// Validates what is checkable without a configuration: finite non-negative distance and
    /// speed, a non-negative slot, no self-draft.
    pub fn new(
        id: RiderId,
        distance_m: f64,
        speed_ms: f64,
        lateral_slot: i64,
        draft_target_id: Option<RiderId>,
    ) -> Result<Self> {
        check_finite_non_negative(distance_m, "distanceM")?;
        check_finite_non_negative(speed_ms, "speedMs")?;
        check_non_negative_int(lateral_slot, "lateralSlot")?;
        if draft_target_id.as_ref() == Some(&id) {
            return Err(RaceSimulationError::validation(
                C::SelfDraftTarget,
                format!("rider \"{id}\" cannot draft itself"),
                "draftTargetId",
            ));
        }
        Ok(Self {
            id,
            distance_m,
            speed_ms,
            lateral_slot: lateral_slot as usize,
            draft_target_id,
        })
    }

    /// Which rider.
    pub fn id(&self) -> &RiderId {
        &self.id
    }
    /// Distance along the course, in metres. Unbounded above.
    pub fn distance_m(&self) -> f64 {
        self.distance_m
    }
    /// Speed, in m/s.
    pub fn speed_ms(&self) -> f64 {
        self.speed_ms
    }
    /// The occupied lateral slot.
    pub fn lateral_slot(&self) -> usize {
        self.lateral_slot
    }
    /// The wheel this rider is on, if any. Part of the continuation state.
    pub fn draft_target_id(&self) -> Option<&RiderId> {
        self.draft_target_id.as_ref()
    }
}

/// The complete dynamic continuation state, including the RNG position.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceState {
    simulation_time: Duration,
    random_state: RandomState,
    riders: Vec<RiderState>,
}

impl RaceState {
    /// Canonicalises riders; duplicates are rejected.
    pub fn new(
        simulation_time: Duration,
        random_state: RandomState,
        mut riders: Vec<RiderState>,
    ) -> Result<Self> {
        riders.sort_by(|a, b| a.id.cmp(&b.id));
        Self::canonical(simulation_time, random_state, riders)
    }

    /// Adopts riders that must already be canonical.
    pub fn canonical(
        simulation_time: Duration,
        random_state: RandomState,
        riders: Vec<RiderState>,
    ) -> Result<Self> {
        check_canonical(&riders, |r| &r.id, "riders", ".id")?;
        Ok(Self {
            simulation_time,
            random_state,
            riders,
        })
    }

    /// Simulation time: a count of fixed steps, never a wall clock.
    pub fn simulation_time(&self) -> Duration {
        self.simulation_time
    }
    /// The generator position.
    pub fn random_state(&self) -> RandomState {
        self.random_state
    }
    /// Riders, canonical.
    pub fn riders(&self) -> &[RiderState] {
        &self.riders
    }

    /// The state of `id`.
    pub fn rider_state(&self, id: &RiderId) -> Option<&RiderState> {
        self.riders
            .binary_search_by(|r| r.id.cmp(id))
            .ok()
            .map(|i| &self.riders[i])
    }
}

/// What one software-simulated rider is doing during one step. The whole controller boundary:
/// power, braking, and two *intentions* (a slot, a wheel) the kernel resolves. A controller can
/// never set speed or position. There is deliberately no human/observed variant (Slice 9 owns
/// reconciling trainer telemetry).
#[derive(Debug, Clone, PartialEq)]
pub struct RiderControlInput {
    id: RiderId,
    power_watts: f64,
    braking_force_n: f64,
    lateral_slot: Option<usize>,
    draft_target_id: Option<RiderId>,
}

impl RiderControlInput {
    /// Validates finite non-negative power and braking, a non-negative slot, and no self-draft.
    pub fn new(
        id: RiderId,
        power_watts: f64,
        braking_force_n: f64,
        lateral_slot: Option<i64>,
        draft_target_id: Option<RiderId>,
    ) -> Result<Self> {
        check_finite_non_negative(power_watts, "powerWatts")?;
        check_finite_non_negative(braking_force_n, "brakingForceN")?;
        if let Some(slot) = lateral_slot {
            check_non_negative_int(slot, "lateralSlot")?;
        }
        if draft_target_id.as_ref() == Some(&id) {
            return Err(RaceSimulationError::validation(
                C::SelfDraftTarget,
                format!("rider \"{id}\" cannot request to draft itself"),
                "draftTargetId",
            ));
        }
        Ok(Self {
            id,
            power_watts,
            braking_force_n,
            lateral_slot: lateral_slot.map(|s| s as usize),
            draft_target_id,
        })
    }

    /// Pedalling only, no requests.
    pub fn power(id: RiderId, power_watts: f64) -> Result<Self> {
        Self::new(id, power_watts, 0.0, None, None)
    }

    /// Which rider.
    pub fn id(&self) -> &RiderId {
        &self.id
    }
    /// Crank power, in watts.
    pub fn power_watts(&self) -> f64 {
        self.power_watts
    }
    /// Braking force, in newtons.
    pub fn braking_force_n(&self) -> f64 {
        self.braking_force_n
    }
    /// The requested slot, if any.
    pub fn lateral_slot(&self) -> Option<usize> {
        self.lateral_slot
    }
    /// The requested wheel, if any.
    pub fn draft_target_id(&self) -> Option<&RiderId> {
        self.draft_target_id.as_ref()
    }
}

/// Every rider's input for one step: total, canonical, one per configured rider.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceInputFrame {
    inputs: Vec<RiderControlInput>,
}

impl RaceInputFrame {
    /// Canonicalises; duplicates are rejected.
    pub fn new(mut inputs: Vec<RiderControlInput>) -> Result<Self> {
        inputs.sort_by(|a, b| a.id.cmp(&b.id));
        check_canonical(&inputs, |i| &i.id, "inputs", ".id")?;
        Ok(Self { inputs })
    }

    /// Inputs, canonical.
    pub fn inputs(&self) -> &[RiderControlInput] {
        &self.inputs
    }

    /// The input for `id`.
    pub fn input_for(&self, id: &RiderId) -> Option<&RiderControlInput> {
        self.inputs
            .binary_search_by(|i| i.id.cmp(id))
            .ok()
            .map(|i| &self.inputs[i])
    }
}

/// What one step produced. Events are a report, never an input.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceStepResult {
    /// The state after the step.
    pub next_state: RaceState,
    /// What happened, in emission order (see [`RaceEventKind`](crate::RaceEventKind)).
    pub events: Vec<RaceEvent>,
}

impl RiderState {
    pub(crate) fn stepped(
        id: RiderId,
        distance_m: f64,
        speed_ms: f64,
        lateral_slot: usize,
        draft_target_id: Option<RiderId>,
    ) -> Result<Self> {
        Self::new(
            id,
            distance_m,
            speed_ms,
            lateral_slot as i64,
            draft_target_id,
        )
    }
}
