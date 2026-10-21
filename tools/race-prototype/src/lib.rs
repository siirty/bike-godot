//! Offline race-action compilation and honest replay diagnostics over the production kernel.
use race_engine::*;
use race_physics::{
    power_for_speed, solve_speed_for_power, step_physiology, EnvironmentParams,
    LongitudinalDynamicsConfig, PhysiologyParams, RiderPhysicalParams, SolverOptions,
};
use serde::Serialize;
use std::time::Duration;

/// Fixed production step (seconds); all phase durations are whole seconds.
pub const DT: f64 = 0.05;
/// Ordered names, matching kernel canonical order.
pub const NAMES: [&str; 5] = ["athlete", "decoy", "pacer", "rival", "support"];
/// Test fixture error. Kernel and argument errors are propagated to the caller.
pub type Error = Box<dyn std::error::Error>;

/// An authored action, not a claim about observed race state.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Phase {
    /// Display name.
    pub name: String,
    /// Engineering power-zone index, Z1–Z7.
    pub zone: u8,
    /// Starting workout time, seconds.
    pub start: u32,
    /// Length, seconds.
    pub duration: u32,
    /// Requested athlete power / FTP.
    pub intensity: f64,
    /// Authored tactical cue.
    pub action: String,
    /// Whether the authored move is intended to matter.
    pub decisive: bool,
    /// Planned road grade (distance-based terrain is frozen separately).
    pub grade: f64,
}
/// Experiment settings, validated before simulation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Options {
    /// Fixture FTP; CP is explicitly set equal to this for this experiment only.
    pub ftp: f64,
    /// Athlete plus bicycle mass, kg.
    pub mass: f64,
    /// Deterministic course/profile seed.
    pub seed: u64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            ftp: 250.0,
            mass: 85.0,
            seed: 42,
        }
    }
}
impl Options {
    /// Reject pathological command-line inputs before constructing any model.
    pub fn validate(&self) -> Result<(), Error> {
        if !self.ftp.is_finite()
            || !(100.0..=450.0).contains(&self.ftp)
            || !self.mass.is_finite()
            || !(50.0..=130.0).contains(&self.mass)
        {
            return Err("fixture FTP must be 100–450 W and total mass 50–130 kg".into());
        }
        Ok(())
    }
}
/// Varied all-zone engineering workout: every zone, work/recovery and a finishing sprint.
pub fn workout() -> Vec<Phase> {
    let rows = [
        ("Settle", 1, 60, 0.45, "Sheltered roll-out", false, 0.0),
        ("Endurance", 2, 120, 0.65, "Hold the bunch", false, 0.005),
        ("Tempo", 3, 90, 0.83, "Take a steady pull", false, 0.01),
        (
            "Threshold",
            4,
            120,
            1.00,
            "Hold the strong move",
            true,
            0.025,
        ),
        (
            "Recover I",
            1,
            75,
            0.45,
            "Shelter; let the weak solo move go",
            false,
            -0.01,
        ),
        (
            "VO2",
            5,
            60,
            1.15,
            "Cover the rival's acceleration",
            true,
            0.02,
        ),
        (
            "Recover II",
            1,
            90,
            0.45,
            "Let the group cover the move",
            false,
            -0.015,
        ),
        ("Anaerobic", 6, 30, 1.40, "Bridge to the rival", true, 0.015),
        ("Recover III", 1, 120, 0.45, "Rejoin shelter", false, -0.01),
        (
            "Tempo again",
            3,
            90,
            0.83,
            "Resume a steady pull",
            false,
            0.005,
        ),
        (
            "Prepare finish",
            2,
            90,
            0.65,
            "Hold a wheel before the finish",
            false,
            0.0,
        ),
        ("Sprint", 7, 15, 2.00, "Contest the finish", true, 0.0),
        (
            "Cool down",
            1,
            60,
            0.40,
            "Race finished; ease off",
            false,
            -0.005,
        ),
    ];
    let mut start = 0;
    rows.into_iter()
        .map(
            |(name, zone, duration, intensity, action, decisive, grade)| {
                let p = Phase {
                    name: name.into(),
                    zone,
                    start,
                    duration,
                    intensity,
                    action: action.into(),
                    decisive,
                    grade,
                };
                start += duration;
                p
            },
        )
        .collect()
}
fn id(i: usize) -> RiderId {
    RiderId::new(NAMES[i]).expect("static id")
}
fn at(phases: &[Phase], seconds: f64) -> usize {
    phases
        .partition_point(|p| f64::from(p.start) <= seconds)
        .saturating_sub(1)
}
fn duration(phases: &[Phase]) -> u32 {
    phases.last().map_or(0, |p| p.start + p.duration)
}
fn environment(config: &RaceSimulationConfig, d: f64) -> EnvironmentParams {
    EnvironmentParams {
        grade_fraction: config.terrain().grade_at(d),
        air_density_kg_m3: config.air_density_kg_m3(),
        headwind_ms: config.headwind_ms(),
    }
}
fn make_config(options: &Options, phases: &[Phase]) -> Result<RaceSimulationConfig, Error> {
    let mut rng = Pcg32::seeded(options.seed, 0x70726f74);
    let riders = (0..5)
        .map(|i| {
            let cp = options.ftp * [1.0, 0.70, 1.12, 1.15, 1.08][i];
            let mass = options.mass + [0.0, -4.0, 2.0, -2.0, 1.0][i];
            let rider = RiderPhysicalParams {
                cda_m2: 0.30 + 0.02 * rng.next_f64(),
                ..RiderPhysicalParams::new(mass)
            };
            Ok(
                RiderDefinition::new(id(i), rider)?.with_physiology(PhysiologyParams::new(
                    cp,
                    20_000.0,
                    options.ftp * 4.5,
                )?),
            )
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let physical = *riders[0].physical();
    let mut samples = Vec::new();
    let mut d = 0.0;
    let mut previous = 0.0;
    for p in phases {
        samples.push(TerrainSample::new(d, previous)?);
        samples.push(TerrainSample::new(d + 30.0, p.grade)?);
        let v = solve_speed_for_power(
            options.ftp * p.intensity,
            &physical,
            &EnvironmentParams::with_grade(p.grade),
            SolverOptions::default(),
        )?;
        d += (v * f64::from(p.duration)).max(60.0);
        previous = p.grade;
    }
    Ok(RaceSimulationConfig::new(RaceSimulationConfigSpec {
        fixed_step: Duration::from_millis(50),
        riders,
        terrain: SimulationTerrain::new(
            samples,
            TerrainInterpolation::Linear,
            BeyondEndBehavior::HoldLastGrade,
            None,
        )?,
        dynamics: LongitudinalDynamicsConfig::default(),
        drafting: Some(BasicDraftingConfig::new(8.0, vec![0.62, 0.85])?),
        lateral_slot_count: 3,
        air_density_kg_m3: 1.225,
        headwind_ms: 0.0,
    })?)
}
fn initial(config: &RaceSimulationConfig, seed: u64) -> Result<RaceState, Error> {
    let gaps = [0.0, -8.0, 4.0, -4.0, 8.0];
    let riders = config
        .riders()
        .iter()
        .enumerate()
        .map(|(i, d)| {
            Ok(RiderState::new(
                d.id().clone(),
                20.0 + gaps[i],
                7.0,
                if i == 1 { 2 } else { 0 },
                None,
            )?
            .with_w_prime_balance(d.physiology().unwrap().w_prime_j())?)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(RaceState::new(
        Duration::ZERO,
        Pcg32::seeded(seed, 1).state(),
        riders,
    )?)
}
/// Frozen opponent intention. No position, capability or target-athlete power is stored here.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct Command {
    /// Crank-power request, W (kernel applies capability limits).
    pub power: f64,
    /// Braking intention, N.
    pub brake: f64,
    /// Lateral intention.
    pub slot: i64,
}
fn command(i: usize, c: Command) -> Result<RiderControlInput, Error> {
    Ok(RiderControlInput::new(
        id(i),
        c.power,
        c.brake,
        Some(c.slot),
        None,
    )?)
}
/// One frozen script over a fixed field and distance-based terrain.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Generation settings.
    pub options: Options,
    /// Authored sequence.
    pub phases: Vec<Phase>,
    /// Immutable physics and capabilities.
    pub config: RaceSimulationConfig,
    /// Common initial state for all replays.
    pub initial: RaceState,
    /// One recorded set of opponent inputs per 50 ms step (decoy, pacer, rival, support).
    pub script: Vec<[Command; 4]>,
    /// Nominal athlete distance at the end of the finishing sprint.
    pub finish_m: f64,
}
fn desired_gap(i: usize, phase: &Phase, elapsed: f64) -> f64 {
    let progress = (elapsed / f64::from(phase.duration)).clamp(0.0, 1.0);
    match i {
        1 => {
            if phase.zone == 1 && phase.start > 0 && phase.start < 900 {
                -8.0 + 24.0 * (std::f64::consts::PI * progress).sin().max(0.0)
            } else {
                -8.0
            }
        }
        2 => {
            if phase.zone <= 2 {
                4.0
            } else {
                -4.0
            }
        }
        3 => {
            if phase.decisive {
                4.0 + 10.0 * (std::f64::consts::PI * progress).sin().max(0.0)
            } else {
                -4.0
            }
        }
        _ => {
            if phase.zone <= 2 {
                8.0
            } else {
                -8.0
            }
        }
    }
}
/// Compile intentions offline against one nominal rider. Only the kernel advances state.
pub fn compile(options: Options) -> Result<Plan, Error> {
    options.validate()?;
    let phases = workout();
    let config = make_config(&options, &phases)?;
    let initial = initial(&config, options.seed)?;
    let sim = RaceSimulation::new(config.clone())?;
    let mut state = initial.clone();
    let mut script = Vec::new();
    let mut finish_m = 0.0;
    for tick in 0..duration(&phases) * 20 {
        let seconds = f64::from(tick) * DT;
        let phase = &phases[at(&phases, seconds)];
        let human = &state.riders()[0];
        let mut cmds = [Command {
            power: 0.0,
            brake: 0.0,
            slot: 0,
        }; 4];
        for (i, c) in cmds.iter_mut().enumerate() {
            let i = i + 1;
            let ai = &state.riders()[i];
            let def = &config.riders()[i];
            let gap = desired_gap(i, phase, seconds - f64::from(phase.start));
            let target_speed = (human.speed_ms()
                + 0.18 * (human.distance_m() + gap - ai.distance_m()))
            .clamp(0.0, 25.0);
            let equilibrium = power_for_speed(
                ai.speed_ms(),
                def.physical(),
                &environment(&config, ai.distance_m()),
            );
            // Offline proportional speed fitting; these become fixed intentions before replay.
            let force = def.physical().total_mass_kg * 0.8 * (target_speed - ai.speed_ms());
            let watts = equilibrium + force * ai.speed_ms() / def.physical().drive_train_efficiency;
            *c = Command {
                power: watts.max(0.0),
                brake: if watts < 0.0 { -force.min(0.0) } else { 0.0 },
                slot: if i == 1 { 2 } else { 0 },
            };
        }
        let mut inputs = vec![RiderControlInput::power(
            id(0),
            phase.intensity * options.ftp,
        )?];
        for (i, c) in cmds.iter().enumerate() {
            inputs.push(command(i + 1, *c)?);
        }
        state = sim
            .step(&state, &RaceInputFrame::new(inputs)?, config.fixed_step())?
            .next_state;
        if phase.zone == 7 {
            finish_m = state.riders()[0].distance_m();
        }
        script.push(cmds);
    }
    let config = RaceSimulationConfig::new(RaceSimulationConfigSpec {
        fixed_step: config.fixed_step(),
        riders: config.riders().to_vec(),
        terrain: SimulationTerrain::new(
            config.terrain().samples().to_vec(),
            config.terrain().interpolation(),
            config.terrain().beyond_end(),
            Some(finish_m),
        )?,
        dynamics: *config.dynamics(),
        drafting: config.drafting().cloned(),
        lateral_slot_count: config.lateral_slot_count() as i64,
        air_density_kg_m3: config.air_density_kg_m3(),
        headwind_ms: config.headwind_ms(),
    })?;
    Ok(Plan {
        options,
        phases,
        config,
        initial,
        script,
        finish_m,
    })
}
/// Replay modes. Every mode uses exactly the same frozen opponent inputs and terrain.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum Mode {
    /// Requested workout effort.
    Prescribed,
    /// 5% less requested power.
    Under,
    /// 5% more requested power.
    Over,
    /// Power changes three seconds late.
    Late,
    /// Follow the named action's wheel with a simple local gap controller, without target watts.
    FollowAction,
    /// Prescribed effort except spending extra power to chase decoy moves during recovery.
    CoverDecoy,
    /// Skip the anaerobic bridge effort deliberately; consequences persist.
    MissBridge,
}
/// Replay variants included in the default report.
pub const MODES: [Mode; 7] = [
    Mode::Prescribed,
    Mode::Under,
    Mode::Over,
    Mode::Late,
    Mode::FollowAction,
    Mode::CoverDecoy,
    Mode::MissBridge,
];
fn athlete_command(plan: &Plan, state: &RaceState, tick: usize, mode: Mode) -> Command {
    let t = tick as f64 * DT;
    let p = &plan.phases[at(&plan.phases, t)];
    let requested = p.intensity * plan.options.ftp;
    let power = match mode {
        Mode::Under => requested * 0.95,
        Mode::Over => requested * 1.05,
        Mode::Late => {
            plan.phases[at(&plan.phases, (t - 3.0).max(0.0))].intensity * plan.options.ftp
        }
        Mode::MissBridge if p.zone == 6 => 0.45 * plan.options.ftp,
        Mode::CoverDecoy if p.zone == 1 && p.start > 0 && p.start < 900 => {
            let ai = &state.riders()[1];
            let h = &state.riders()[0];
            (requested + 35.0 * (ai.distance_m() - h.distance_m() - 4.0).max(0.0))
                .min(plan.options.ftp * 1.5)
        }
        Mode::FollowAction => {
            let h = &state.riders()[0];
            let target = &state.riders()[if p.decisive { 3 } else { 2 }];
            let physical = plan.config.riders()[0].physical();
            let required = power_for_speed(
                h.speed_ms(),
                physical,
                &environment(&plan.config, h.distance_m()),
            );
            (required
                + 25.0
                    * (target.distance_m() - h.distance_m() - if p.zone == 3 { -4.0 } else { 4.0 })
                + 50.0 * (target.speed_ms() - h.speed_ms()))
            .clamp(0.0, plan.options.ftp * 3.0)
        }
        _ => requested,
    };
    Command {
        power,
        brake: 0.0,
        slot: 0,
    }
}
/// Semantic observation derived from geometry alone, independently of the workout schedule.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum ObservedScene {
    /// A useful wheel is available.
    Sheltered,
    /// Strong opposition is ahead within 8–25 m, outside useful shelter.
    Chasing,
    /// No strong opponent remains within 25 m ahead or 8 m behind.
    DetachedBehind,
    /// Strong opposition is all more than 8 m behind.
    SoloAhead,
    /// At the front with strong riders nearby behind.
    Leading,
    /// Nearby riders without an acquired wheel or a clear chase/lead.
    InGroup,
}
fn classify(state: &RaceState) -> ObservedScene {
    let h = &state.riders()[0];
    let gaps: Vec<_> = [2, 3, 4]
        .into_iter()
        .map(|i| state.riders()[i].distance_m() - h.distance_m())
        .collect();
    if gaps.iter().all(|g| *g > 25.0) {
        return ObservedScene::DetachedBehind;
    }
    if gaps.iter().all(|g| *g < -8.0) {
        return ObservedScene::SoloAhead;
    }
    if h.draft_target_id().is_some() {
        return ObservedScene::Sheltered;
    }
    if gaps.iter().any(|g| *g > 8.0 && *g <= 25.0) {
        return ObservedScene::Chasing;
    }
    if gaps.iter().all(|g| *g <= 0.0) && gaps.iter().any(|g| *g >= -8.0) {
        return ObservedScene::Leading;
    }
    ObservedScene::InGroup
}
/// An observed scene transition, confirmed after two seconds of persistent geometry.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Transition {
    /// Confirmation time, seconds.
    pub t: f64,
    /// Previous observed scene.
    pub from: ObservedScene,
    /// New observed scene.
    pub to: ObservedScene,
}
/// Sampled observed geometry and measured simulated effort. Never written to activity history.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Sample {
    /// Debounced semantic observation, not the authored phase name.
    pub scene: ObservedScene,
    /// Workout time, seconds.
    pub t: f64,
    /// Scheduled phase index (not an observed scene classification).
    pub phase: usize,
    /// Actual athlete distance, m.
    pub distance: f64,
    /// Requested workout watts.
    pub target: f64,
    /// Actual delivered athlete watts after physiology.
    pub power: f64,
    /// Mechanical watts to hold the named target's speed/acceleration with current geometry.
    pub demand: f64,
    /// Observed road grade.
    pub grade: f64,
    /// Positions relative to athlete in canonical rider order, m.
    pub gaps: [f64; 5],
    /// Actual rider speeds, m/s.
    pub speeds: [f64; 5],
    /// Actual reserve fractions.
    pub reserves: [f64; 5],
    /// Available wheel selected by the kernel, if any.
    pub wheel: Option<String>,
}
/// Per-phase outcome with explicit engineering gates, independent of scheduled scene labels.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Outcome {
    /// Scheduled phase.
    pub phase: usize,
    /// Delivered mean athlete power.
    pub mean_power: f64,
    /// Mean mechanical demand at the named target.
    pub mean_demand: f64,
    /// Fraction within ±10% FTP of prescribed effort (not a validated training metric).
    pub intensity_share: f64,
    /// Fraction of the phase actually within the named target's usable wheel range.
    pub wheel_share: f64,
    /// Fraction in the intended formation: sheltered, pulling, or within the authored move.
    pub formation_share: f64,
    /// First time within the intended wheel range, seconds into the phase.
    pub acquisition_s: Option<f64>,
    /// Target-relative gap when the phase begins, m.
    pub entry_gap: f64,
    /// Target-relative gap when the phase ends, m.
    pub exit_gap: f64,
    /// Physiology-limited power shortfall, joules.
    pub shortfall_j: f64,
    /// Lowest remaining athlete reserve fraction.
    pub min_reserve: f64,
    /// Observed geometry supports the authored demand, under provisional gates.
    pub passed: bool,
    /// Explain failures instead of relabeling the observed scene as successful.
    pub reasons: Vec<String>,
}
/// A finish reported by the kernel; placement is an outcome, not an authored result.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Finish {
    /// Rider identifier.
    pub rider: String,
    /// Actual finishing place.
    pub place: u32,
    /// Crossing time at fixed-step resolution.
    pub time_s: f64,
}
/// Full replay result for playback and machine inspection.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Run {
    /// Actual kernel finishing order, including opponents.
    pub finishers: Vec<Finish>,
    /// Scene changes classified from observed geometry, independent of phase timing.
    pub transitions: Vec<Transition>,
    /// Reference policy used.
    pub mode: Mode,
    /// Recorded one-second samples.
    pub samples: Vec<Sample>,
    /// Outcomes computed at 20 Hz, not downsampled from playback.
    pub outcomes: Vec<Outcome>,
    /// Athlete's crossing time at the frozen sprint finish, if reached.
    pub finish_s: Option<f64>,
    /// Actual kernel event count.
    pub events: usize,
}
#[derive(Default)]
struct Acc {
    n: u32,
    power: f64,
    demand: f64,
    intensity: u32,
    wheel: u32,
    formation: u32,
    first: Option<f64>,
    entry: f64,
    exit: f64,
    shortfall: f64,
    min_reserve: f64,
}
/// Execute a frozen plan without feeding athlete errors back into opponent commands.
pub fn replay(plan: &Plan, mode: Mode) -> Result<Run, Error> {
    let sim = RaceSimulation::new(plan.config.clone())?;
    let mut state = plan.initial.clone();
    let mut sums: Vec<Acc> = plan
        .phases
        .iter()
        .map(|_| Acc {
            min_reserve: 1.0,
            ..Default::default()
        })
        .collect();
    let mut samples = Vec::new();
    let mut finish_s = None;
    let mut events = 0;
    let mut finishers = Vec::new();
    let mut observed = classify(&state);
    let mut pending = observed;
    let mut pending_ticks = 0;
    let mut transitions = Vec::new();
    for (tick, cmds) in plan.script.iter().enumerate() {
        let t = tick as f64 * DT;
        let index = at(&plan.phases, t);
        let p = &plan.phases[index];
        let candidate = classify(&state);
        if candidate == observed {
            pending_ticks = 0;
        } else {
            if candidate != pending {
                pending = candidate;
                pending_ticks = 0;
            }
            pending_ticks += 1;
            if pending_ticks >= 40 {
                transitions.push(Transition {
                    t,
                    from: observed,
                    to: candidate,
                });
                observed = candidate;
                pending_ticks = 0;
            }
        }
        let hcmd = athlete_command(plan, &state, tick, mode);
        let h = &state.riders()[0];
        let target_index = if p.decisive { 3 } else { 2 };
        let target = &state.riders()[target_index];
        let mut inputs = vec![command(0, hcmd)?];
        for (i, c) in cmds.iter().enumerate() {
            inputs.push(command(i + 1, *c)?);
        }
        let frame = RaceInputFrame::new(inputs)?;
        let slots = race_engine::simulation::positioning::resolve_lateral_slots(&state, &frame);
        let targets = race_engine::simulation::positioning::select_draft_targets(
            &plan.config,
            &state,
            &slots,
            &frame,
        );
        let drags = race_engine::simulation::positioning::compute_drag_multipliers(
            &plan.config,
            &state,
            &slots,
            &targets,
        );
        let physiology = plan.config.riders()[0].physiology().unwrap();
        let delivered = step_physiology(
            physiology,
            h.w_prime_balance_j().unwrap(),
            hcmd.power,
            plan.config.fixed_step(),
        )?
        .delivered_power_w;
        let result = sim.step(&state, &frame, plan.config.fixed_step())?;
        plan.config
            .validate_state(&result.next_state, "prototype.next")?;
        for (a, b) in state.riders().iter().zip(result.next_state.riders()) {
            if b.distance_m() < a.distance_m() {
                return Err("kernel moved rider backwards".into());
            }
        }
        let accel = (result.next_state.riders()[target_index].speed_ms() - target.speed_ms()) / DT;
        let mut physical = *plan.config.riders()[0].physical();
        physical.cda_m2 *= drags[0];
        let v = target.speed_ms();
        let demand = (power_for_speed(v, &physical, &environment(&plan.config, h.distance_m()))
            + physical.total_mass_kg * accel * v / physical.drive_train_efficiency)
            .max(0.0);
        let gap = target.distance_m() - h.distance_m();
        let wheel = gap > 2.5 && gap < 8.0 && targets[0].as_ref() == Some(target.id());
        let a = &mut sums[index];
        if a.n == 0 {
            a.entry = gap;
        }
        a.n += 1;
        a.power += delivered;
        a.demand += demand;
        a.exit = gap;
        a.intensity +=
            u32::from((delivered - p.intensity * plan.options.ftp).abs() <= 0.1 * plan.options.ftp);
        let formed = if p.zone <= 2 {
            wheel
        } else if p.zone == 3 {
            (-15.0..0.0).contains(&gap)
        } else {
            (0.0..25.0).contains(&gap)
        };
        a.formation += u32::from(formed);
        a.wheel += u32::from(wheel);
        if formed && a.first.is_none() {
            a.first = Some(t - f64::from(p.start));
        }
        a.shortfall += (hcmd.power - delivered).max(0.0) * DT;
        a.min_reserve = a.min_reserve.min(
            result.next_state.riders()[0].w_prime_balance_j().unwrap() / physiology.w_prime_j(),
        );
        if finish_s.is_none() && result.next_state.riders()[0].distance_m() >= plan.finish_m {
            finish_s = Some(t + DT);
        }
        for event in &result.events {
            if let RaceEventKind::Finished { rider, place } = event.kind() {
                finishers.push(Finish {
                    rider: rider.as_str().into(),
                    place: *place,
                    time_s: event.simulation_time().as_secs_f64(),
                });
            }
        }
        events += result.events.len();
        if tick % 20 == 0 {
            samples.push(Sample {
                scene: observed,
                t,
                phase: index,
                distance: h.distance_m(),
                target: p.intensity * plan.options.ftp,
                power: delivered,
                demand,
                grade: plan.config.terrain().grade_at(h.distance_m()),
                gaps: std::array::from_fn(|i| state.riders()[i].distance_m() - h.distance_m()),
                speeds: std::array::from_fn(|i| state.riders()[i].speed_ms()),
                reserves: std::array::from_fn(|i| {
                    state.riders()[i].w_prime_balance_j().unwrap()
                        / plan.config.riders()[i].physiology().unwrap().w_prime_j()
                }),
                wheel: h.draft_target_id().map(|id| id.as_str().into()),
            });
        }
        state = result.next_state;
    }
    let outcomes = sums
        .into_iter()
        .enumerate()
        .map(|(i, a)| {
            let n = f64::from(a.n);
            let p = &plan.phases[i];
            let mean_demand = a.demand / n;
            let mut reasons = Vec::new();
            if a.intensity as f64 / n < 0.8 {
                reasons.push(
                    "Delivered effort misses the ±10% FTP engineering band for >20% of phase"
                        .into(),
                );
            }
            if (mean_demand - p.intensity * plan.options.ftp).abs() > 0.20 * plan.options.ftp {
                reasons.push(
                    "Target-wheel mechanical demand differs from prescription by >20% FTP".into(),
                );
            }
            if a.formation as f64 / n < 0.5 {
                reasons.push("Intended formation absent for more than half the phase".into());
            }
            if a.shortfall > 0.01 * p.intensity * plan.options.ftp * f64::from(p.duration) {
                reasons.push("Requested effort exceeds remaining physiological capability".into());
            }
            Outcome {
                phase: i,
                mean_power: a.power / n,
                mean_demand,
                intensity_share: a.intensity as f64 / n,
                wheel_share: a.wheel as f64 / n,
                formation_share: a.formation as f64 / n,
                acquisition_s: a.first,
                entry_gap: a.entry,
                exit_gap: a.exit,
                shortfall_j: a.shortfall,
                min_reserve: a.min_reserve,
                passed: reasons.is_empty(),
                reasons,
            }
        })
        .collect();
    Ok(Run {
        finishers,
        transitions,
        mode,
        samples,
        outcomes,
        finish_s,
        events,
    })
}
/// Complete export. Script is stored separately so invariance can be inspected directly.
#[derive(Serialize)]
pub struct Report {
    /// Versioned experimental report, not a kernel snapshot or activity file.
    pub version: u32,
    /// Fixed generation settings.
    pub options: Options,
    /// Stable rider names.
    pub riders: [&'static str; 5],
    /// Fixed CP, W′ and peak power in rider order.
    pub capabilities: Vec<[f64; 3]>,
    /// Workout/action schedule.
    pub phases: Vec<Phase>,
    /// Frozen finish distance.
    pub finish_m: f64,
    /// Distance/grade terrain samples.
    pub terrain: Vec<[f64; 2]>,
    /// All replay outcomes.
    pub runs: Vec<Run>,
}
impl Plan {
    /// Run all comparison policies against this single compiled schedule.
    pub fn report(&self) -> Result<Report, Error> {
        Ok(Report {
            version: 1,
            options: self.options.clone(),
            riders: NAMES,
            capabilities: self
                .config
                .riders()
                .iter()
                .map(|d| {
                    let p = d.physiology().unwrap();
                    [p.critical_power_w(), p.w_prime_j(), p.peak_power_w()]
                })
                .collect(),
            phases: self.phases.clone(),
            finish_m: self.finish_m,
            terrain: self
                .config
                .terrain()
                .samples()
                .iter()
                .map(|s| [s.distance_m(), s.grade_fraction()])
                .collect(),
            runs: MODES
                .into_iter()
                .map(|mode| replay(self, mode))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
