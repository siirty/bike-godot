//! Mixed-session engineering fixture. All movement uses the existing production kernel.
//! Pace intentions are fitted offline, then frozen. Runtime opponents never see target error.
use crate::{observation::*, simulation::positioning, tactics::*, *};
use race_physics::{
    power_for_speed, step_physiology, EnvironmentParams, LongitudinalDynamicsConfig,
    PhysiologyParams, RiderPhysicalParams,
};
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use workout_import::WorkoutTimeline;

/// Canonical persistent roster identities.
pub const RIDERS: [&str; 6] = ["alex", "human", "independent", "mara", "pacer", "rival"];
/// Software player identity, never a source of measured activity.
pub const PLAYER: &str = "human";
/// Explicit engineering fixture settings, fixed before riding.
#[derive(Debug, Clone, Copy)]
pub struct MixedOptions {
    ftp: f64,
    seed: u64,
    cooperative: bool,
}
impl MixedOptions {
    /// The fixture supports a bounded FTP range; CP equals FTP only for this experiment.
    pub fn new(ftp: f64, seed: u64, cooperative: bool) -> Result<Self, MixedError> {
        if !ftp.is_finite() || !(100.0..=450.0).contains(&ftp) {
            return Err(MixedError::Invalid(
                "FTP must be finite and between 100 and 450 W",
            ));
        }
        Ok(Self {
            ftp,
            seed,
            cooperative,
        })
    }
}
impl Default for MixedOptions {
    fn default() -> Self {
        Self::new(300.0, 42, true).unwrap()
    }
}
/// Validated-control and kernel errors.
#[derive(Debug, thiserror::Error)]
pub enum MixedError {
    /// Existing kernel validation failure.
    #[error(transparent)]
    Kernel(#[from] RaceSimulationError),
    /// Existing spotlight-policy validation failure.
    #[error(transparent)]
    Tactics(#[from] TacticsError),
    /// Invalid fixture, control or continuation.
    #[error("mixed session: {0}")]
    Invalid(&'static str),
}
/// Continuation annotation, separate from the observed race state and future opponent roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Scene {
    /// Prepare for the competitive exercise.
    WarmUp,
    /// Physically available shelter.
    HoldShelter,
    /// Contribute toward an observed rider ahead.
    Pursue,
    /// Maintain a contribution with company nearby.
    MaintainContribution,
    /// No usable wheel: continue independently, with no automatic rescue.
    SoloFallback,
    /// Measured sustained work.
    SteadyContribution,
    /// Brief contribution.
    ShortResponse,
    /// End the competitive exercise while cooling down.
    CoolDown,
}
/// Inspector values are simulated data, never athlete telemetry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RiderSample {
    /// Identity.
    pub id: String,
    /// Actual position.
    pub distance_m: f64,
    /// Actual speed.
    pub speed_ms: f64,
    /// Kernel-resolved slot.
    pub slot: usize,
    /// Actual eligible draft target.
    pub wheel: Option<String>,
    /// Finite reserve.
    pub reserve_j: f64,
    /// Ordinary requested power, possibly physiology limited.
    pub requested_w: f64,
}
/// End-of-step trace, with input block/target from the start of that step.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sample {
    /// Elapsed simulation time.
    pub time_s: f64,
    /// Half-open workout block index.
    pub block: usize,
    /// Prescribed software target, separate from delivered work.
    pub target_w: f64,
    /// Physiology-limited delivered power.
    pub delivered_w: f64,
    /// Provisional diagnostic band, ±5% FTP.
    pub in_band: bool,
    /// Authored continuation annotation.
    pub scene: Scene,
    /// Bounded role requested from spotlight AI, not a successful-action claim.
    pub spotlight_role: String,
    /// Independent geometric projection.
    pub observation: Observation,
    /// Canonical rider order.
    pub riders: Vec<RiderSample>,
}
/// Exact ordinary inputs and events can be independently replayed through RaceSimulation.
pub struct MixedStep {
    /// Total fixed-step input frame.
    pub inputs: RaceInputFrame,
    /// Actual kernel events.
    pub events: Vec<RaceEvent>,
    /// Inspector projection.
    pub sample: Sample,
}
/// Pure deterministic session continuation, shared by Godot and headless execution.
#[derive(Debug, Clone)]
pub struct MixedSession {
    options: MixedOptions,
    simulation: RaceSimulation,
    state: RaceState,
    timeline: WorkoutTimeline,
    pace: Arc<Vec<(f64, f64)>>,
    spotlight: TacticalController,
    observer: RaceObserver,
    observation: Observation,
    last: Option<Sample>,
}
fn id(name: &str) -> RiderId {
    RiderId::new(name).unwrap()
}
fn spotlight() -> TacticalController {
    TacticalController::new(
        id("independent"),
        TacticsConfig::new(TacticsSpec {
            cruise_cp: 0.60,
            recovery_cp: 0.55,
            ..TacticsSpec::default()
        })
        .unwrap(),
        TacticalProfile::new(0.2, 0.9, 0.9).unwrap(),
    )
}
fn role(block: usize) -> TacticalRole {
    match block {
        2 => TacticalRole::Chaser,
        5 => TacticalRole::Instigator,
        4 => TacticalRole::Neutral,
        _ => TacticalRole::Conserving,
    }
}
fn block_at(time: Duration) -> usize {
    let s = time.as_secs();
    [600, 1500, 1680, 2280, 2640, 2700, 3300]
        .into_iter()
        .filter(|end| s >= *end)
        .count()
}

// Ordinary local motion control. Only own capability and observable geometry are inputs.
fn track(
    config: &RaceSimulationConfig,
    state: &RaceState,
    name: &str,
    speed: f64,
    error: f64,
) -> RiderControlInput {
    let i = state
        .riders()
        .iter()
        .position(|r| r.id().as_str() == name)
        .unwrap();
    let me = &state.riders()[i];
    let own = &config.riders()[i];
    let controls = RaceInputFrame::new(
        state
            .riders()
            .iter()
            .map(|r| RiderControlInput::new(r.id().clone(), 0.0, 0.0, Some(1), None).unwrap())
            .collect(),
    )
    .unwrap();
    let slots = positioning::resolve_lateral_slots(state, &controls);
    let wheels = positioning::select_draft_targets(config, state, &slots, &controls);
    let drags = positioning::compute_drag_multipliers(config, state, &slots, &wheels);
    let mut physical = *own.physical();
    physical.cda_m2 *= drags[i];
    let environment = EnvironmentParams {
        grade_fraction: config.terrain().grade_at(me.distance_m()),
        air_density_kg_m3: config.air_density_kg_m3(),
        headwind_ms: config.headwind_ms(),
    };
    let steady = power_for_speed(me.speed_ms(), &physical, &environment);
    let desired = (speed + (0.12 * error).clamp(-1.0, 1.0)).max(0.0);
    let watts = steady
        + physical.total_mass_kg * (desired - me.speed_ms()) / 2.0 * me.speed_ms().max(0.5)
            / physical.drive_train_efficiency;
    let brake = if watts < 0.0 && me.speed_ms() > 0.1 {
        -watts * physical.drive_train_efficiency / me.speed_ms()
    } else {
        0.0
    };
    RiderControlInput::new(
        me.id().clone(),
        watts.clamp(0.0, own.physiology().unwrap().peak_power_w()),
        brake,
        Some(1),
        None,
    )
    .unwrap()
}
// Frozen own attack windows plus local following/recovery; no workout argument.
fn pack_input(
    config: &RaceSimulationConfig,
    state: &RaceState,
    name: &str,
    pace: (f64, f64),
) -> RiderControlInput {
    let me = state.rider_state(&id(name)).unwrap();
    let cap = config
        .definition_for(me.id())
        .unwrap()
        .physiology()
        .unwrap();
    let cp = cap.critical_power_w();
    let power = |w| RiderControlInput::new(me.id().clone(), w, 0.0, Some(1), None).unwrap();
    if name == "pacer" {
        return RiderControlInput::new(me.id().clone(), pace.0, pace.1, Some(1), None).unwrap();
    }
    if me.w_prime_balance_j().unwrap() < 0.2 * cap.w_prime_j() {
        return power(cp * 0.6);
    }
    let t = state.simulation_time().as_secs_f64();
    let attack = (name == "alex" && (720.0..760.0).contains(&t))
        || (name == "rival" && ((1485.0..1530.0).contains(&t) || (2640.0..2700.0).contains(&t)));
    if attack
        && state
            .riders()
            .iter()
            .any(|r| r.id() != me.id() && (r.distance_m() - me.distance_m()).abs() < 40.0)
    {
        return power(cp * 1.3);
    }
    let target = state
        .rider_state(&id(match name {
            "alex" => "mara",
            "mara" => "human",
            _ => "pacer",
        }))
        .unwrap();
    if target.distance_m() < me.distance_m() {
        power(cp * 0.62)
    } else {
        track(
            config,
            state,
            name,
            target.speed_ms(),
            target.distance_m() - me.distance_m() - 4.2,
        )
    }
}
impl MixedSession {
    /// Fit only the supporting pace offline; freeze it before accepting any runtime controls.
    pub fn new(options: MixedOptions) -> Result<Self, MixedError> {
        let riders = RIDERS
            .into_iter()
            .map(|name| {
                let ratio = match name {
                    "pacer" => 1.13,
                    "rival" => 1.08,
                    "mara" => 0.97,
                    "independent" => 1.02,
                    _ => 1.0,
                };
                let peak = match name {
                    "mara" => 4.5,
                    "alex" => 3.6,
                    "rival" => 3.7,
                    "pacer" => 3.3,
                    "independent" => 3.8,
                    _ => 4.0,
                };
                RiderDefinition::new(id(name), RiderPhysicalParams::new(85.0))
                    .unwrap()
                    .with_physiology(
                        PhysiologyParams::new(options.ftp * ratio, 20000.0, options.ftp * peak)
                            .unwrap(),
                    )
            })
            .collect();
        let terrain = SimulationTerrain::new(
            [
                (0.0, 0.0),
                (4000.0, 0.008),
                (7000.0, -0.005),
                (14000.0, 0.002),
                (18000.0, 0.012),
                (24000.0, 0.005),
                (25500.0, 0.02),
                (30000.0, -0.005),
                (40000.0, 0.0),
            ]
            .into_iter()
            .map(|(d, g)| TerrainSample::new(d, g).unwrap())
            .collect(),
            TerrainInterpolation::Linear,
            BeyondEndBehavior::HoldLastGrade,
            None,
        )?;
        let config = RaceSimulationConfig::new(RaceSimulationConfigSpec {
            fixed_step: Duration::from_millis(50),
            riders,
            terrain,
            dynamics: LongitudinalDynamicsConfig::default(),
            drafting: Some(BasicDraftingConfig::new(6.0, vec![0.62, 0.85])?),
            lateral_slot_count: 3,
            air_density_kg_m3: 1.225,
            headwind_ms: 0.0,
        })?;
        let state = RaceState::new(
            Duration::ZERO,
            Pcg32::seeded(options.seed, 1).state(),
            RIDERS
                .into_iter()
                .map(|name| {
                    RiderState::new(
                        id(name),
                        match name {
                            "pacer" => 124.0,
                            "human" => 120.0,
                            "mara" => 116.0,
                            "alex" => 112.0,
                            "rival" => 108.0,
                            _ => 104.0,
                        },
                        8.0,
                        1,
                        None,
                    )
                    .unwrap()
                    .with_w_prime_balance(20000.0)
                    .unwrap()
                })
                .collect(),
        )?;
        let simulation = RaceSimulation::new(config)?;
        let timeline =
            WorkoutTimeline::new(session_director::mixed::workout()).expect("valid fixture");
        let mut nominal = state.clone();
        let mut actor = spotlight();
        let mut pace = Vec::with_capacity(72000);
        for _ in 0..72000 {
            let block = block_at(nominal.simulation_time());
            let human = nominal.rider_state(&id(PLAYER)).unwrap();
            let jo = nominal.rider_state(&id("pacer")).unwrap();
            let offset = match block {
                2 => -8.0,
                4 => -4.2,
                5 => -6.0,
                _ => 4.2,
            };
            let command = if block == 2 && !options.cooperative {
                RiderControlInput::power(id("pacer"), options.ftp * 1.13 * 0.6)?
            } else {
                track(
                    simulation.config(),
                    &nominal,
                    "pacer",
                    human.speed_ms(),
                    human.distance_m() + offset - jo.distance_m(),
                )
            };
            let p = (command.power_watts(), command.braking_force_n());
            pace.push(p);
            let watts = timeline
                .power_at(nominal.simulation_time(), options.ftp)
                .unwrap()
                .unwrap();
            let inputs = Self::inputs(
                simulation.config(),
                &nominal,
                &mut actor,
                block,
                p,
                RiderControlInput::new(id(PLAYER), watts, 0.0, Some(1), None)?,
            )?;
            nominal = simulation
                .step(&nominal, &inputs, simulation.config().fixed_step())?
                .next_state;
        }
        let mut observer = RaceObserver::default();
        let observation = observer
            .observe(simulation.config(), &state, ChaseIntent::None)
            .map_err(MixedError::Invalid)?;
        Ok(Self {
            options,
            simulation,
            state,
            timeline,
            pace: Arc::new(pace),
            spotlight: spotlight(),
            observer,
            observation,
            last: None,
        })
    }
    fn inputs(
        config: &RaceSimulationConfig,
        state: &RaceState,
        actor: &mut TacticalController,
        block: usize,
        pace: (f64, f64),
        player: RiderControlInput,
    ) -> Result<RaceInputFrame, MixedError> {
        let mut inputs = vec![player];
        for name in RIDERS.into_iter().filter(|n| *n != PLAYER) {
            inputs.push(if name == "independent" {
                actor.input(config, state, role(block))?
            } else {
                pack_input(config, state, name, pace)
            });
        }
        Ok(RaceInputFrame::new(inputs)?)
    }
    /// Immutable kernel configuration.
    pub fn config(&self) -> &RaceSimulationConfig {
        self.simulation.config()
    }
    /// Authoritative simulated state.
    pub fn state(&self) -> &RaceState {
        &self.state
    }
    /// Frozen options for replay validation.
    pub fn options(&self) -> (f64, u64, bool) {
        (
            self.options.ftp,
            self.options.seed,
            self.options.cooperative,
        )
    }
    /// Current half-open block index.
    pub fn block(&self) -> usize {
        block_at(self.state.simulation_time())
    }
    /// Fixed session duration, not a fabricated finish distance.
    pub fn is_complete(&self) -> bool {
        self.state.simulation_time() >= self.timeline.total_duration()
    }
    /// Current software target; never written to a trainer.
    pub fn target_w(&self) -> f64 {
        self.timeline
            .power_at(self.state.simulation_time(), self.options.ftp)
            .unwrap()
            .unwrap()
    }
    /// Most recent completed step.
    pub fn last_sample(&self) -> Option<&Sample> {
        self.last.as_ref()
    }
    /// Current geometric observation.
    pub fn observation(&self) -> &Observation {
        &self.observation
    }
    /// One ordinary step. Validate controls before advancing any policy continuation.
    pub fn step(
        &mut self,
        watts: f64,
        slot: usize,
        wheel: Option<&str>,
    ) -> Result<MixedStep, MixedError> {
        if self.is_complete() {
            return Err(MixedError::Invalid("session is complete"));
        }
        if slot >= 3 {
            return Err(MixedError::Invalid("road slot outside range"));
        }
        let player = RiderControlInput::new(
            id(PLAYER),
            watts,
            0.0,
            Some(slot as i64),
            wheel.map(RiderId::new).transpose()?,
        )?;
        let placeholders = RaceInputFrame::new(
            RIDERS
                .into_iter()
                .map(|n| {
                    if n == PLAYER {
                        player.clone()
                    } else {
                        RiderControlInput::power(id(n), 0.0).unwrap()
                    }
                })
                .collect(),
        )?;
        self.config()
            .validate_input_frame(&placeholders, "mixed.player")?;
        let block = self.block();
        let target_w = self.target_w();
        let me = self.state.rider_state(&id(PLAYER)).unwrap();
        let ahead = self
            .state
            .riders()
            .iter()
            .filter(|r| r.id() != me.id() && r.distance_m() > me.distance_m())
            .map(|r| r.distance_m() - me.distance_m())
            .min_by(f64::total_cmp);
        let scene = match block {
            0 => Scene::WarmUp,
            7 => Scene::CoolDown,
            _ if ahead.is_none_or(|g| g > 50.0) => Scene::SoloFallback,
            2 if ahead.is_some_and(|g| g > 12.0) => Scene::Pursue,
            2 => Scene::MaintainContribution,
            4 => Scene::SteadyContribution,
            5 => Scene::ShortResponse,
            _ if me.draft_target_id().is_some() => Scene::HoldShelter,
            _ => Scene::SoloFallback,
        };
        let delivered_w = step_physiology(
            self.config()
                .definition_for(me.id())
                .unwrap()
                .physiology()
                .unwrap(),
            me.w_prime_balance_j().unwrap(),
            watts,
            self.config().fixed_step(),
        )
        .map_err(|_| MixedError::Invalid("invalid physiology input"))?
        .delivered_power_w;
        let p = self.pace[(self.state.simulation_time().as_millis() / 50) as usize];
        let inputs = Self::inputs(
            self.simulation.config(),
            &self.state,
            &mut self.spotlight,
            block,
            p,
            player,
        )?;
        let next = self
            .simulation
            .step(&self.state, &inputs, self.config().fixed_step())?;
        // Only an explicit supporting commitment from group two concerns this relationship.
        let groups = self.observation.groups();
        let intent = if block == 2
            && self.options.cooperative
            && groups.len() > 1
            && groups[1].members().iter().any(|n| n == "pacer")
        {
            ChaseIntent::Pursue
        } else {
            ChaseIntent::None
        };
        let observation = self
            .observer
            .observe(self.simulation.config(), &next.next_state, intent)
            .map_err(MixedError::Invalid)?;
        let sample = Sample {
            time_s: next.next_state.simulation_time().as_secs_f64(),
            block,
            target_w,
            delivered_w,
            in_band: (delivered_w - target_w).abs() <= 0.05 * self.options.ftp,
            scene,
            spotlight_role: format!("{:?}", role(block)),
            observation: observation.clone(),
            riders: next
                .next_state
                .riders()
                .iter()
                .map(|r| RiderSample {
                    id: r.id().as_str().into(),
                    distance_m: r.distance_m(),
                    speed_ms: r.speed_ms(),
                    slot: r.lateral_slot(),
                    wheel: r.draft_target_id().map(|w| w.as_str().into()),
                    reserve_j: r.w_prime_balance_j().unwrap(),
                    requested_w: inputs.input_for(r.id()).unwrap().power_watts(),
                })
                .collect(),
        };
        self.state = next.next_state;
        self.observation = observation;
        self.last = Some(sample.clone());
        Ok(MixedStep {
            inputs,
            events: next.events,
            sample,
        })
    }
}
