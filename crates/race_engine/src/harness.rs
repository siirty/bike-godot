//! The headless harness: seeded race scenarios, a runner that checks the kernel's invariants on
//! every step, and a corpus report.
//!
//! Everything here drives the one production kernel ([`RaceSimulation::step`]); nothing
//! re-implements a step. Scenarios are *inputs*: a seed fixes the field, the course and every
//! rider's power plan, so a run is reproducible from the seed alone. The plans are scripted test
//! inputs, not race AI: they never look at other riders.
//!
//! ```sh
//! cargo run -p race_engine --features testing --example headless_corpus --release -- 0 32
//! ```

use std::fmt::Write as _;
use std::ops::Range;
use std::time::Duration;

use race_physics::{LongitudinalDynamicsConfig, RiderPhysicalParams};

use crate::{
    BasicDraftingConfig, BeyondEndBehavior, Pcg32, RaceEvent, RaceEventKind, RaceInputFrame,
    RaceSimulation, RaceSimulationConfig, RaceSimulationConfigSpec, RaceSimulationError, RaceState,
    RiderControlInput, RiderDefinition, RiderId, RiderState, SimulationTerrain,
    TerrainInterpolation, TerrainSample,
};

/// The PCG stream scenario generation draws from. Fixed, so a seed means the same scenario forever.
pub const CORPUS_STREAM: u64 = 0x5eed;

/// Why a run stopped early.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HarnessError {
    /// The kernel rejected a step.
    #[error("step {step}: {source}")]
    Simulation {
        /// The step index (0-based) that failed.
        step: u64,
        /// The kernel's error.
        source: RaceSimulationError,
    },
    /// The kernel accepted a step whose result breaks an accepted invariant.
    #[error("step {step}: invariant broken: {message}")]
    Invariant {
        /// The step index (0-based) whose result broke it.
        step: u64,
        /// What broke.
        message: String,
    },
}

/// The outcome of one run.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceRun {
    /// The state after the last step.
    pub final_state: RaceState,
    /// Every event, in emission order.
    pub events: Vec<RaceEvent>,
    /// How many steps ran.
    pub steps: u64,
    /// Rider-steps that ended with a draft target, out of `rider_steps`.
    pub drafting_rider_steps: u64,
    /// Riders × steps.
    pub rider_steps: u64,
}

impl RaceRun {
    /// `(rider, place, time)` for every finisher, by place.
    pub fn finishers(&self) -> Vec<(RiderId, u32, Duration)> {
        let mut out: Vec<_> = self
            .events
            .iter()
            .filter_map(|e| match e.kind() {
                RaceEventKind::Finished { rider, place } => {
                    Some((rider.clone(), *place, e.simulation_time()))
                }
                _ => None,
            })
            .collect();
        out.sort_by_key(|f| f.1);
        out
    }

    /// How many events of the kind `matches` selects.
    pub fn count(&self, matches: impl Fn(&RaceEventKind) -> bool) -> usize {
        self.events.iter().filter(|e| matches(e.kind())).count()
    }
}

/// Steps `simulation` from `initial` until every rider has finished (when the course has a
/// length) or `max_steps` ran, asking `inputs` for each step's frame.
///
/// After every step it checks what the contracts promise and the kernel does not re-check on its
/// own output: time advanced by exactly the fixed step, no rider moved backwards, every distance
/// and speed is finite and non-negative, and events carry the post-step time in emission-kind
/// order.
pub fn run_race(
    simulation: &RaceSimulation,
    initial: RaceState,
    max_steps: u64,
    mut inputs: impl FnMut(&RaceState) -> Result<RaceInputFrame, RaceSimulationError>,
) -> Result<RaceRun, HarnessError> {
    let config = simulation.config();
    let dt = config.fixed_step();
    let has_finish = config.terrain().explicit_length_m().is_some();
    let riders = initial.riders().len();
    let mut state = initial;
    let mut events = Vec::new();
    let mut finished = 0usize;
    let mut drafting_rider_steps = 0u64;
    let mut steps = 0u64;

    while steps < max_steps && !(has_finish && finished == riders) {
        let frame = inputs(&state).map_err(|source| HarnessError::Simulation {
            step: steps,
            source,
        })?;
        let result =
            simulation
                .step(&state, &frame, dt)
                .map_err(|source| HarnessError::Simulation {
                    step: steps,
                    source,
                })?;
        config
            .validate_state(&result.next_state, "nextState")
            .map_err(|source| HarnessError::Simulation {
                step: steps,
                source,
            })?;
        check_step(&state, &result.next_state, &result.events, dt).map_err(|message| {
            HarnessError::Invariant {
                step: steps,
                message,
            }
        })?;
        finished += result
            .events
            .iter()
            .filter(|e| matches!(e.kind(), RaceEventKind::Finished { .. }))
            .count();
        drafting_rider_steps += result
            .next_state
            .riders()
            .iter()
            .filter(|r| r.draft_target_id().is_some())
            .count() as u64;
        events.extend(result.events);
        state = result.next_state;
        steps += 1;
    }

    Ok(RaceRun {
        final_state: state,
        events,
        steps,
        drafting_rider_steps,
        rider_steps: steps * riders as u64,
    })
}

fn check_step(
    pre: &RaceState,
    post: &RaceState,
    events: &[RaceEvent],
    dt: Duration,
) -> Result<(), String> {
    if post.simulation_time() != pre.simulation_time() + dt {
        return Err(format!(
            "time went from {:?} to {:?} with a {dt:?} step",
            pre.simulation_time(),
            post.simulation_time()
        ));
    }
    for (before, after) in pre.riders().iter().zip(post.riders()) {
        let (d, v) = (after.distance_m(), after.speed_ms());
        if !d.is_finite() || !v.is_finite() || v < 0.0 {
            return Err(format!("{}: distance {d}, speed {v}", after.id()));
        }
        if d < before.distance_m() {
            return Err(format!(
                "{} moved backwards: {} → {d}",
                after.id(),
                before.distance_m()
            ));
        }
    }
    let mut last_kind = 0;
    for e in events {
        if e.simulation_time() != post.simulation_time() {
            return Err(format!("event {:?} not stamped with the post-step time", e));
        }
        let kind = kind_index(e.kind());
        if kind < last_kind {
            return Err(format!("event {:?} out of emission order", e));
        }
        last_kind = kind;
    }
    Ok(())
}

fn kind_index(kind: &RaceEventKind) -> u8 {
    match kind {
        RaceEventKind::SlotChanged { .. } => 0,
        RaceEventKind::WheelLost { .. } => 1,
        RaceEventKind::WheelTaken { .. } => 2,
        RaceEventKind::Overtook { .. } => 3,
        RaceEventKind::Finished { .. } => 4,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PlanSegment {
    start_step: u64,
    lateral_slot: Option<usize>,
}

/// One seeded race: field, course, start and every rider's scripted power plan.
#[derive(Debug, Clone, PartialEq)]
pub struct CorpusScenario {
    /// The seed it was generated from.
    pub seed: u64,
    /// The configuration to simulate.
    pub config: RaceSimulationConfig,
    /// The start state.
    pub initial_state: RaceState,
    /// A generous step limit: the course at 4 m/s.
    pub max_steps: u64,
    plans: Vec<Vec<PlanSegment>>,
    pace_plans: Vec<crate::pack::PackPacePlan>,
}

impl CorpusScenario {
    /// The scenario for `seed`.
    ///
    /// - 6–10 riders, 62–88 kg total mass, CdA 0.30–0.37 m², and a sustainable power of
    ///   3.0–4.3 W/kg of body mass (total minus 9 kg of bike);
    /// - a 6–10 km course: a bounded random walk of grades in [−4 %, +7 %], sampled every 400 m
    ///   and ramped linearly;
    /// - a line start 1.5 m apart at 8 m/s in random slots of three;
    /// - seeded 30-second pack pace blocks bounded by fixed CP/W′/Pmax, plus a 25% chance
    ///   per block of a lateral-slot request; capability values are provisional fixtures.
    pub fn generate(seed: u64) -> Result<Self, RaceSimulationError> {
        let mut rng = Pcg32::seeded(seed, CORPUS_STREAM);
        let step = Duration::from_millis(50);
        let steps_per_second = 20u64;

        let rider_count = 6 + rng.next_int_below(5)? as usize;
        let mut definitions = Vec::with_capacity(rider_count);
        let mut sustainable = Vec::with_capacity(rider_count);
        for i in 0..rider_count {
            let mass = 62.0 + 26.0 * rng.next_f64();
            let physical = RiderPhysicalParams {
                cda_m2: 0.30 + 0.07 * rng.next_f64(),
                ..RiderPhysicalParams::new(mass)
            };
            sustainable.push((3.0 + 1.3 * rng.next_f64()) * (mass - 9.0));
            definitions.push(
                RiderDefinition::new(RiderId::new(format!("r{:02}", i + 1))?, physical)?
                    .with_physiology(
                        race_physics::PhysiologyParams::new(
                            *sustainable.last().expect("one CP per rider"),
                            16_000.0 + 800.0 * i as f64,
                            900.0 + 40.0 * i as f64,
                        )
                        .expect("valid capability fixture"),
                    ),
            );
        }

        let length_m = (6000.0 + 4000.0 * rng.next_f64()).round();
        let mut samples = Vec::new();
        let mut grade: f64 = 0.0;
        let mut at = 0.0;
        while at < length_m {
            samples.push(TerrainSample::new(at, grade)?);
            grade = (grade + (rng.next_f64() - 0.5) * 0.03).clamp(-0.04, 0.07);
            at += 400.0;
        }
        let terrain = SimulationTerrain::new(
            samples,
            TerrainInterpolation::Linear,
            BeyondEndBehavior::Flat,
            Some(length_m),
        )?;

        let slots = 3usize;
        let mut order: Vec<usize> = (0..rider_count).collect();
        for i in (1..order.len()).rev() {
            let j = rng.next_int_below(i as u64 + 1)? as usize;
            order.swap(i, j);
        }
        let mut riders = Vec::with_capacity(rider_count);
        for (position, &rider) in order.iter().enumerate() {
            riders.push(
                RiderState::new(
                    definitions[rider].id().clone(),
                    1.5 * position as f64,
                    8.0,
                    rng.next_int_below(slots as u64)? as i64,
                    None,
                )?
                .with_w_prime_balance(
                    definitions[rider]
                        .physiology()
                        .expect("fixture physiology")
                        .w_prime_j(),
                )?,
            );
        }

        let max_steps = (length_m / 4.0).ceil() as u64 * steps_per_second;
        let mut plans = Vec::with_capacity(rider_count);
        for _ in &sustainable {
            let mut plan = Vec::new();
            let mut start = 0u64;
            while start < max_steps {
                let slot = (rng.next_f64() < 0.25)
                    .then(|| rng.next_int_below(slots as u64))
                    .transpose()?
                    .map(|s| s as usize);
                plan.push(PlanSegment {
                    start_step: start,
                    lateral_slot: slot,
                });
                start += 30 * steps_per_second;
            }
            plans.push(plan);
        }

        let config = RaceSimulationConfig::new(RaceSimulationConfigSpec {
            fixed_step: step,
            riders: definitions,
            terrain,
            dynamics: LongitudinalDynamicsConfig::default(),
            drafting: Some(BasicDraftingConfig::new(6.0, vec![0.62, 0.85])?),
            lateral_slot_count: slots as i64,
            air_density_kg_m3: 1.225,
            headwind_ms: 0.0,
        })?;
        let initial_state = RaceState::new(Duration::ZERO, Pcg32::seeded(seed, 1).state(), riders)?;
        // Plans follow the configuration's canonical rider order (IDs were generated in order).
        Ok(Self {
            seed,
            config,
            initial_state,
            max_steps,
            plans,
            pace_plans: (0..rider_count)
                .map(|i| crate::pack::PackPacePlan::seeded(seed.wrapping_add(i as u64)))
                .collect(),
        })
    }

    /// The frame for `state`. A pure function of the state's simulation time, so a race resumed
    /// from a snapshot receives exactly the inputs an uninterrupted one would.
    pub fn inputs_at(&self, state: &RaceState) -> Result<RaceInputFrame, RaceSimulationError> {
        let step =
            (state.simulation_time().as_micros() / self.config.fixed_step().as_micros()) as u64;
        let inputs = self
            .config
            .riders()
            .iter()
            .zip(&self.plans)
            .enumerate()
            .map(|(index, (rider, plan))| {
                let i = plan.partition_point(|s| s.start_step <= step).max(1) - 1;
                let segment = &plan[i];
                // A slot request applies on the block's first step only; afterwards the rider holds.
                let slot = segment
                    .lateral_slot
                    .filter(|_| segment.start_step == step)
                    .map(|s| s as i64);
                RiderControlInput::new(
                    rider.id().clone(),
                    self.pace_plans[index].requested_power(
                        rider,
                        state.rider_state(rider.id()).expect("configured rider"),
                        state.simulation_time(),
                    )?,
                    0.0,
                    slot,
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        RaceInputFrame::new(inputs)
    }

    /// Runs the scenario to the finish.
    pub fn run(&self) -> Result<RaceRun, HarnessError> {
        let simulation = RaceSimulation::new(self.config.clone())
            .map_err(|source| HarnessError::Simulation { step: 0, source })?;
        run_race(
            &simulation,
            self.initial_state.clone(),
            self.max_steps,
            |s| self.inputs_at(s),
        )
    }
}

/// One seed's summary.
#[derive(Debug, Clone, PartialEq)]
pub struct SeedReport {
    /// The seed.
    pub seed: u64,
    /// Field size.
    pub riders: usize,
    /// Course length.
    pub length_m: f64,
    /// Whether every rider finished inside the step limit.
    pub all_finished: bool,
    /// Winner's time.
    pub winning_time: Duration,
    /// Last finisher's time minus the winner's.
    pub spread: Duration,
    /// Average speed of the winner.
    pub winner_kmh: f64,
    /// Overtakes.
    pub overtakes: usize,
    /// The most overtakes between any one pair, in either direction. A high value means two riders
    /// leapfrog at near-equal speed instead of one riding away: a believability signal.
    pub max_pair_swaps: usize,
    /// Wheels taken.
    pub wheels_taken: usize,
    /// Slot changes.
    pub slot_changes: usize,
    /// Share of rider-steps spent drafting.
    pub drafting_share: f64,
}

/// Generates and runs every seed in `seeds`.
pub fn run_corpus(seeds: Range<u64>) -> Result<Vec<SeedReport>, HarnessError> {
    seeds
        .map(|seed| {
            let scenario = CorpusScenario::generate(seed)
                .map_err(|source| HarnessError::Simulation { step: 0, source })?;
            let run = scenario.run()?;
            let finishers = run.finishers();
            let length_m = scenario.config.terrain().end_distance_m();
            let winning_time = finishers.first().map_or(Duration::ZERO, |f| f.2);
            let last = finishers.last().map_or(Duration::ZERO, |f| f.2);
            Ok(SeedReport {
                seed,
                riders: scenario.config.riders().len(),
                length_m,
                all_finished: finishers.len() == scenario.config.riders().len(),
                winning_time,
                spread: last - winning_time,
                winner_kmh: length_m / winning_time.as_secs_f64().max(f64::MIN_POSITIVE) * 3.6,
                overtakes: run.count(|k| matches!(k, RaceEventKind::Overtook { .. })),
                max_pair_swaps: max_pair_swaps(&run),
                wheels_taken: run.count(|k| matches!(k, RaceEventKind::WheelTaken { .. })),
                slot_changes: run.count(|k| matches!(k, RaceEventKind::SlotChanged { .. })),
                drafting_share: run.drafting_rider_steps as f64 / run.rider_steps.max(1) as f64,
            })
        })
        .collect()
}

fn max_pair_swaps(run: &RaceRun) -> usize {
    let mut pairs: std::collections::BTreeMap<(&RiderId, &RiderId), usize> = Default::default();
    for e in &run.events {
        if let RaceEventKind::Overtook { rider, passed } = e.kind() {
            *pairs
                .entry((rider.min(passed), rider.max(passed)))
                .or_default() += 1;
        }
    }
    pairs.into_values().max().unwrap_or(0)
}

/// A markdown table of `reports`, then the means.
pub fn render_corpus_report(reports: &[SeedReport]) -> String {
    let mut out = String::from(
        "| seed | riders | km | finished | winner | km/h | spread | overtakes | worst pair | wheels | slots | drafting |\n\
         |---:|---:|---:|:---:|---:|---:|---:|---:|---:|---:|---:|---:|\n",
    );
    for r in reports {
        let _ = writeln!(
            out,
            "| {} | {} | {:.1} | {} | {} | {:.1} | {:.1} s | {} | {} | {} | {} | {:.0} % |",
            r.seed,
            r.riders,
            r.length_m / 1000.0,
            if r.all_finished { "yes" } else { "NO" },
            clock(r.winning_time),
            r.winner_kmh,
            r.spread.as_secs_f64(),
            r.overtakes,
            r.max_pair_swaps,
            r.wheels_taken,
            r.slot_changes,
            r.drafting_share * 100.0
        );
    }
    let n = reports.len().max(1) as f64;
    let mean = |f: &dyn Fn(&SeedReport) -> f64| reports.iter().map(f).sum::<f64>() / n;
    let _ = writeln!(
        out,
        "\n{} seeds, {} finished in full. Means: {:.1} km/h winner, {:.1} s spread, {:.0} overtakes, \
         {:.0} wheels taken, {:.0} % of rider-time drafting.",
        reports.len(),
        reports.iter().filter(|r| r.all_finished).count(),
        mean(&|r| r.winner_kmh),
        mean(&|r| r.spread.as_secs_f64()),
        mean(&|r| r.overtakes as f64),
        mean(&|r| r.wheels_taken as f64),
        mean(&|r| r.drafting_share * 100.0),
    );
    out
}

fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}
