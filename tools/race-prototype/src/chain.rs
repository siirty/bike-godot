//! Focused connected-situation experiment. Offline fitting is separate from runtime policies.
use crate::{command, environment, make_config, motor, Command, Error, Options, Phase, DT, NAMES};
use race_engine::perception::{read_move, MoveDecision, PublicRaceView, ReaderSettings};
use race_engine::*;
use race_physics::{solve_speed_for_power, step_physiology, SolverOptions};
use serde::Serialize;
use session_director::situations::{LearningPlan, PlayerKnowledge, PlayerObjective};

/// Explicit perturbations; these are execution tests, not proficiency labels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Execution {
    /// Continuous public-cue execution.
    Cue,
    /// Hold each control command for one second.
    Delayed,
    /// Alternate ±3% requested power every five seconds.
    Noisy,
    /// Scale motor output by 95%, preserving actual consequences.
    Under,
    /// Scale motor output by 105%, preserving actual consequences.
    Over,
    /// Deliberately skip the second threshold effort.
    MissSecond,
    /// Cover the other-file movement whenever it opens beyond 8 m.
    Cover,
}
/// Fixed reference bounds checked before accepting the engineering fixture.
pub const EXECUTIONS: [Execution; 7] = [
    Execution::Cue,
    Execution::Delayed,
    Execution::Noisy,
    Execution::Under,
    Execution::Over,
    Execution::MissSecond,
    Execution::Cover,
];
/// A frozen course and bounded opponent pace intentions. No live workout-error feedback.
#[derive(Debug, Clone)]
pub struct ChainPlan {
    /// Fixed generation inputs.
    options: Options,
    /// Frozen course, physics and capabilities.
    config: RaceSimulationConfig,
    /// Common initial physical state.
    initial: RaceState,
    /// Authored contracts and separate public knowledge.
    learning: LearningPlan,
    /// Contiguous engineering workout.
    phases: Vec<Phase>,
    /// Offline-fit pace intentions, m/s. Opponents track these using their own capability.
    pace_ms: Vec<f64>,
}
/// One-second observable trace, sampled after each completed second.
#[derive(Debug, Clone, Serialize)]
pub struct Trace {
    /// Simulation time in seconds.
    pub time_s: f64,
    /// Authored phase index.
    pub phase: usize,
    /// Actual physiology-delivered athlete power.
    pub delivered_w: f64,
    /// Observed deficit to the furthest rider.
    pub gap_to_front_m: f64,
    /// Own remaining W′ fraction.
    pub reserve: f64,
    /// Actual athlete distance.
    pub distance_m: f64,
    /// Actual athlete lateral file.
    pub lateral_slot: usize,
    /// Reference public-cue classification, not a human score.
    pub reading: MoveDecision,
    /// Public motions, suitable for independent inspection.
    pub view: PublicRaceView,
}
/// Independent workload and geometry evidence at actual step boundaries.
#[derive(Debug, Clone, Serialize)]
pub struct StepEvidence {
    /// Authored phase index.
    pub phase: usize,
    /// Fraction of 20 Hz delivered samples inside the authored envelope.
    pub effort_share: f64,
    /// Mean actual delivered watts over the workout step.
    pub mean_w: f64,
    /// Actual fraction with a wheel 0–8 m ahead, independent of authored rider identity.
    pub contact_share: f64,
    /// Actual deficit at the step boundary.
    pub exit_deficit_m: f64,
    /// Own reserve fraction at the step boundary.
    pub reserve_at_exit: f64,
}
/// Continuous replay evidence for one execution policy.
#[derive(Debug, Clone, Serialize)]
pub struct ChainRun {
    /// Execution perturbation used for this replay.
    pub execution: Execution,
    /// Independent per-step workload and transition evidence.
    pub steps: Vec<StepEvidence>,
    /// Actual athlete finish event, if observed.
    pub finish_s: Option<f64>,
    /// Actual athlete work through the replay horizon, kJ.
    pub work_kj: f64,
    /// Actual physiology clipping, not an expected archetype/ranking rule.
    pub shortfall_kj: Vec<f64>,
    /// Per-rider climbing work measured on this course (grade >1%).
    pub climbing_work_kj: Vec<f64>,
    /// Actual kernel finish events and places.
    pub finishers: Vec<crate::Finish>,
    /// One-second observable trace.
    pub trace: Vec<Trace>,
}
/// Pre-ride engineering gates; not a human learning or empirical plausibility verdict.
#[derive(Debug, Clone, Serialize)]
pub struct Acceptance {
    /// Each bounded execution must match both threshold efforts and recovery.
    pub workload: bool,
    /// Contact at the next effort, rather than a whole-run 60 m average.
    pub next_entry_contact: bool,
    /// Finish must follow the final effort's useful duration, within the declared window.
    pub finish_window: bool,
    /// Scheduled pace leaders must be able to execute the course without >100 J clipping.
    pub leader_capacity: bool,
    /// Conjunction of all declared pre-ride acceptance gates.
    pub accepted: bool,
    /// Explicit rejection reasons; empty only when all gates pass.
    pub reasons: Vec<String>,
}
/// Exportable evidence, including rejected plans and intentional rider errors.
#[derive(Debug, Clone, Serialize)]
pub struct ChainReport {
    /// Export schema version.
    pub version: u32,
    /// Fixed generation inputs.
    pub options: Options,
    /// Authored contracts and separate public knowledge.
    pub learning: LearningPlan,
    /// Frozen offline-fit opponent pace intentions.
    pub pace_ms: Vec<f64>,
    /// Frozen course length.
    pub finish_m: f64,
    /// Pre-ride decision for the declared execution bounds.
    pub acceptance: Acceptance,
    /// All nominal, bounded and deliberate-error replays.
    pub runs: Vec<ChainRun>,
    /// Evidence limitations, separate from acceptance.
    pub limitations: Vec<String>,
}
fn phases() -> Vec<Phase> {
    let mut start = 0;
    [
        ("Hold the initial group", 2, 30, 0.65),
        ("Follow the sustained selection", 4, 90, 1.0),
        ("Shelter while Jo takes over", 1, 120, 0.45),
        ("Follow the next selection", 4, 90, 1.0),
    ]
    .into_iter()
    .map(|(name, zone, duration, intensity)| {
        let p = Phase {
            name: name.into(),
            zone,
            start,
            duration,
            intensity,
            action: name.into(),
            decisive: zone == 4,
            grade: 0.015,
        };
        start += duration;
        p
    })
    .collect()
}
fn with_finish(
    config: &RaceSimulationConfig,
    finish: Option<f64>,
) -> Result<RaceSimulationConfig, Error> {
    Ok(RaceSimulationConfig::new(RaceSimulationConfigSpec {
        fixed_step: config.fixed_step(),
        riders: config.riders().to_vec(),
        terrain: SimulationTerrain::new(
            vec![TerrainSample::new(0.0, 0.015)?],
            TerrainInterpolation::Linear,
            BeyondEndBehavior::HoldLastGrade,
            finish,
        )?,
        dynamics: *config.dynamics(),
        drafting: config.drafting().cloned(),
        lateral_slot_count: config.lateral_slot_count() as i64,
        air_density_kg_m3: config.air_density_kg_m3(),
        headwind_ms: config.headwind_ms(),
    })?)
}
// The exercise starts in an already moving group. This is the common pre-ride initial
// state, never a live repositioning or a reset between workout steps.
fn group_start(config: &RaceSimulationConfig, seed: u64, speed: f64) -> Result<RaceState, Error> {
    Ok(RaceState::new(
        std::time::Duration::ZERO,
        Pcg32::seeded(seed, 1).state(),
        config
            .riders()
            .iter()
            .enumerate()
            .map(|(i, r)| {
                Ok(RiderState::new(
                    r.id().clone(),
                    20.0 + [0.0, -8.0, 4.0, 12.0, 8.0][i],
                    speed,
                    if i == 1 { 2 } else { 0 },
                    None,
                )?
                .with_w_prime_balance(r.physiology().unwrap().w_prime_j())?)
            })
            .collect::<Result<Vec<_>, Error>>()?,
    )?)
}
/// Compile a short selection/recovery/selection chain; all fitting happens before replay.
/// Eight bounded fitting passes are allowed. A failed acceptance remains a rejected plan.
pub fn compile(options: Options) -> Result<ChainPlan, Error> {
    options.validate()?;
    let phases = phases();
    let config = with_finish(&make_config(&options, &phases)?, None)?;
    let mut learning = crate::learning::author(&phases, options.ftp)?;
    let k = learning.knowledge();
    let knowledge=PlayerKnowledge::new(k.player().into(),PlayerObjective::StayWithGroup,
        "Stay with this training group through two sustained selections on a steady 1.5% climb. You and Jo ride for Green; Alex and Morgan ride for Blue, and Sam for Gold. Known strengths are shown in the roster, but reputation alone does not settle the importance of a move. Watch who is at the front, relative speed and the available wheel. The line ends this group exercise; no finishing sprint is requested. Synthetic profiles, not a calibrated racing class.".into(),k.riders().to_vec())?;
    learning = LearningPlan::new(knowledge, learning.situations().to_vec())?;
    let mut physical = *config.riders()[0].physical();
    physical.cda_m2 *= 0.62;
    let pace_ms = phases
        .iter()
        .map(|p| {
            solve_speed_for_power(
                options.ftp * p.intensity,
                &physical,
                &environment(&config, 0.0),
                SolverOptions::default(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let initial = group_start(&config, options.seed, pace_ms[0])?;
    let mut plan = ChainPlan {
        options,
        config,
        initial,
        learning,
        phases,
        pace_ms,
    };
    for _ in 0..8 {
        plan.initial = group_start(&plan.config, plan.options.seed, plan.pace_ms[0])?;
        let run = replay(&plan, Execution::Cue)?;
        for i in 0..plan.phases.len() {
            let target = plan.phases[i].intensity * plan.options.ftp;
            let end = f64::from(plan.phases[i].start + plan.phases[i].duration);
            let tail: Vec<_> = run
                .trace
                .iter()
                .filter(|s| s.time_s > end - 20.0 && s.time_s <= end)
                .collect();
            let steady = tail.iter().map(|s| s.delivered_w).sum::<f64>() / tail.len() as f64;
            plan.pace_ms[i] *= (target / steady.max(1.0)).powf(0.22).clamp(0.94, 1.06);
        }
    }
    plan.initial = group_start(&plan.config, plan.options.seed, plan.pace_ms[0])?;
    let reference = replay(&plan, Execution::Cue)?;
    let finish = reference
        .trace
        .last()
        .ok_or("empty chain trace")?
        .distance_m;
    plan.config = with_finish(&plan.config, Some(finish))?;
    Ok(plan)
}
/// Public-cue action selection, separate from the motion executor. No phase/effort input.
fn actor(
    config: &RaceSimulationConfig,
    state: &RaceState,
    k: &PlayerKnowledge,
    cover: bool,
) -> Result<Command, Error> {
    let view = PublicRaceView::observe(config, state, k.player())?;
    let read = read_move(k, &view, ReaderSettings::default())?;
    let h = &state.riders()[0];
    if read.decision == MoveDecision::Finished {
        return Ok(Command {
            power: 0.0,
            brake: 0.0,
            slot: 0,
        });
    }
    let target = if cover {
        view.riders
            .iter()
            .find(|r| r.id == NAMES[1] && r.gap_m > 8.0)
    } else {
        None
    }
    .or_else(|| {
        // StayWithGroup follows the nearest available wheel, including represented opposition.
        // A lone distant target requires the reader's Respond classification.
        view.riders
            .iter()
            .filter(|r| r.gap_m > 0.0 && r.gap_m <= 12.0 && r.lateral_slot == h.lateral_slot())
            .min_by(|a, b| a.gap_m.total_cmp(&b.gap_m).then(a.id.cmp(&b.id)))
    })
    .or_else(|| {
        if read.decision == MoveDecision::Respond {
            view.riders
                .iter()
                .find(|r| Some(&r.id) == read.target.as_ref())
        } else {
            None
        }
    });
    motor::track(
        config,
        state,
        k.player(),
        motor::MotionGoal {
            speed_ms: target.map_or(h.speed_ms(), |r| h.speed_ms() + r.relative_speed_ms),
            error_m: target.map_or(0.0, |r| r.gap_m - 4.0),
            slot: target.map_or(h.lateral_slot(), |r| r.lateral_slot) as i64,
        },
    )
}
/// Local group roles: fixed pace leader plus followers of that leader. No athlete target/error.
fn opponents(
    config: &RaceSimulationConfig,
    state: &RaceState,
    pace_ms: f64,
    leader: usize,
    background_offset_m: f64,
) -> Result<Vec<Command>, Error> {
    let front = &state.riders()[leader];
    (1..5)
        .map(|i| {
            let r = &state.riders()[i];
            let offset = if i == leader {
                0.0
            } else if leader == 2 {
                match i {
                    1 => background_offset_m,
                    3 => -8.0,
                    _ => -4.0,
                }
            } else {
                match i {
                    1 => -12.0,
                    2 => -8.0,
                    _ => -4.0,
                }
            };
            // Jo keeps the group's pace during Alex's other-file movement.
            // Actual capacity failures remain observable, rather than being repaired.
            let (speed, error) = if i == leader {
                (pace_ms, 0.0)
            } else {
                (
                    front.speed_ms(),
                    front.distance_m() + offset - r.distance_m(),
                )
            };
            motor::track(
                config,
                state,
                NAMES[i],
                motor::MotionGoal {
                    speed_ms: speed,
                    error_m: error,
                    slot: if i == 1 { 2 } else { 0 },
                },
            )
        })
        .collect()
}
/// Continuous production-kernel replay with actual fatigue; no regroup/reset between steps.
pub fn replay(plan: &ChainPlan, execution: Execution) -> Result<ChainRun, Error> {
    let sim = RaceSimulation::new(plan.config.clone())?;
    let mut state = plan.initial.clone();
    let workout_ticks = 330 * 20;
    let horizon = workout_ticks
        + if plan.config.terrain().explicit_length_m().is_some() {
            100
        } else {
            0
        };
    let mut sums = [(0u32, 0u32, 0u32, 0.0); 4];
    let mut steps = Vec::new();
    let mut trace = Vec::new();
    let mut finishers = Vec::new();
    let mut work_kj = 0.0;
    let mut shortfall_kj = vec![0.0; 5];
    let mut climbing_work_kj = vec![0.0; 5];
    let mut held = actor(&plan.config, &state, plan.learning.knowledge(), false)?;
    let mut finish_s = None;
    for tick in 0..horizon {
        let t = f64::from(tick) * DT;
        let index = crate::at(&plan.phases, t);
        let view = PublicRaceView::observe(&plan.config, &state, NAMES[0])?;
        if execution != Execution::Delayed || tick % 20 == 0 {
            held = actor(
                &plan.config,
                &state,
                plan.learning.knowledge(),
                execution == Execution::Cover,
            )?;
        }
        let mut h = held;
        match execution {
            Execution::Noisy => h.power *= if (tick / 100) % 2 == 0 { 0.97 } else { 1.03 },
            Execution::Under => h.power *= 0.95,
            Execution::Over => h.power *= 1.05,
            Execution::MissSecond if index == 3 => h.power = 0.45 * plan.options.ftp,
            _ => {}
        }
        let mut cmds = vec![h];
        // The fixture scheduler emits bounded roles; local controllers receive no workout.
        let leader = if index == 2 { 2 } else { 3 };
        let background_offset_m = if index == 2 {
            -12.0
                + 24.0
                    * (std::f64::consts::PI * ((t - 120.0) / 120.0))
                        .sin()
                        .max(0.0)
        } else {
            -12.0
        };
        cmds.extend(opponents(
            &plan.config,
            &state,
            plan.pace_ms[index],
            leader,
            background_offset_m,
        )?);
        let inputs = RaceInputFrame::new(
            cmds.iter()
                .enumerate()
                .map(|(i, c)| command(i, *c))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let result = sim.step(&state, &inputs, plan.config.fixed_step())?;
        plan.config
            .validate_state(&result.next_state, "chain.next")?;
        let mut delivered = 0.0;
        for i in 0..5 {
            let actual = step_physiology(
                plan.config.riders()[i].physiology().unwrap(),
                state.riders()[i].w_prime_balance_j().unwrap(),
                cmds[i].power,
                plan.config.fixed_step(),
            )?
            .delivered_power_w;
            if plan
                .config
                .terrain()
                .explicit_length_m()
                .is_none_or(|end| state.riders()[i].distance_m() < end)
            {
                shortfall_kj[i] += (cmds[i].power - actual).max(0.0) * DT / 1000.0;
                if plan
                    .config
                    .terrain()
                    .grade_at(state.riders()[i].distance_m())
                    > 0.01
                {
                    climbing_work_kj[i] += actual * DT / 1000.0;
                }
                if i == 0 {
                    work_kj += actual * DT / 1000.0;
                }
            }
            if i == 0 {
                delivered = actual;
            }
        }
        let a = &mut sums[index];
        if tick < workout_ticks {
            a.0 += 1;
            a.1 += u32::from(
                plan.learning.situations()[index]
                    .effort()
                    .contains(delivered),
            );
            a.2 += u32::from(view.riders.iter().any(|r| {
                r.gap_m > 0.0
                    && r.gap_m <= 8.0
                    && r.lateral_slot == state.riders()[0].lateral_slot()
            }));
            a.3 += delivered;
        }
        for e in &result.events {
            if let RaceEventKind::Finished {
                rider: rider_id,
                place,
            } = e.kind()
            {
                finishers.push(crate::Finish {
                    rider: rider_id.as_str().into(),
                    time_s: result.next_state.simulation_time().as_secs_f64(),
                    place: *place,
                });
                if rider_id.as_str() == NAMES[0] {
                    finish_s = Some(result.next_state.simulation_time().as_secs_f64());
                }
            }
        }
        state = result.next_state;
        let front = state
            .riders()
            .iter()
            .map(|r| r.distance_m())
            .fold(0.0, f64::max);
        if tick % 20 == 19 {
            let view = PublicRaceView::observe(&plan.config, &state, NAMES[0])?;
            let reading = read_move(plan.learning.knowledge(), &view, ReaderSettings::default())?;
            trace.push(Trace {
                time_s: f64::from(tick + 1) * DT,
                phase: index,
                delivered_w: delivered,
                gap_to_front_m: front - state.riders()[0].distance_m(),
                reserve: state.riders()[0].w_prime_balance_j().unwrap() / 20_000.0,
                distance_m: state.riders()[0].distance_m(),
                lateral_slot: state.riders()[0].lateral_slot(),
                reading: reading.decision,
                view,
            });
        }
        if tick + 1 == workout_ticks
            || (tick < workout_ticks && crate::at(&plan.phases, f64::from(tick + 1) * DT) != index)
        {
            steps.push(StepEvidence {
                phase: index,
                effort_share: f64::from(a.1) / f64::from(a.0),
                mean_w: a.3 / f64::from(a.0),
                contact_share: f64::from(a.2) / f64::from(a.0),
                exit_deficit_m: front - state.riders()[0].distance_m(),
                reserve_at_exit: state.riders()[0].w_prime_balance_j().unwrap() / 20_000.0,
            });
        }
    }
    Ok(ChainRun {
        execution,
        steps,
        finish_s,
        work_kj,
        shortfall_kj,
        climbing_work_kj,
        finishers,
        trace,
    })
}
/// Gate the frozen plan against declared normal execution, retaining failed evidence.
pub fn assess(runs: &[ChainRun]) -> Acceptance {
    let bounded = [Execution::Cue, Execution::Delayed, Execution::Noisy];
    let references: Vec<_> = bounded
        .iter()
        .filter_map(|m| runs.iter().find(|r| r.execution == *m))
        .collect();
    let workload = references.len() == bounded.len()
        && references
            .iter()
            .all(|r| r.steps.len() == 4 && r.steps.iter().all(|s| s.effort_share >= 0.8));
    let next_entry_contact = references.len() == bounded.len()
        && references.iter().all(|r| {
            r.trace.iter().find(|s| s.time_s == 240.0).is_some_and(|s| {
                s.view
                    .riders
                    .iter()
                    .any(|r| r.gap_m > 0.0 && r.gap_m <= 8.0 && r.lateral_slot == 0)
            })
        });
    let finish_window = references.len() == bounded.len()
        && references
            .iter()
            .all(|r| r.finish_s.is_some_and(|t| (323.0..=335.0).contains(&t)));
    let leader_capacity = references.len() == bounded.len()
        && references.iter().all(|r| {
            [2, 3].into_iter().all(|i| {
                r.shortfall_kj
                    .get(i)
                    .is_some_and(|v| v.is_finite() && *v <= 0.1)
            })
        });
    let mut reasons = Vec::new();
    if !workload {
        reasons
            .push("One or more workout envelopes failed in a declared bounded execution.".into());
    }
    if !next_entry_contact {
        reasons.push(
            "The next effort was entered outside the declared 8 m available-wheel bound.".into(),
        );
    }
    if !finish_window {
        reasons.push(
            "A bounded execution finished outside 323–335 s or did not reach the line.".into(),
        );
    }
    if !leader_capacity {
        reasons.push("A scheduled pace leader could not execute this course within its frozen capacity (>100 J clipping).".into());
    }
    Acceptance {
        workload,
        next_entry_contact,
        finish_window,
        leader_capacity,
        accepted: workload && next_entry_contact && finish_window && leader_capacity,
        reasons,
    }
}
impl ChainPlan {
    /// Evaluate every policy without altering the frozen plan.
    pub fn report(&self) -> Result<ChainReport, Error> {
        let runs = EXECUTIONS
            .iter()
            .map(|e| replay(self, *e))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ChainReport{version:1,options:self.options.clone(),learning:self.learning.clone(),pace_ms:self.pace_ms.clone(),finish_m:self.config.terrain().explicit_length_m().unwrap(),acceptance:assess(&runs),runs,limitations:vec!["Synthetic 1.5% climbing group exercise, not a full race or empirical proficiency model.".into(),"Public identities, motion and leader changes support inspection; human recognition/transfer remains untested.".into(),"Pace fitting is offline; runtime opponents use only frozen roles and actual motion. No target-error rescue.".into(),"Entry contact is checked at the next effort, and both actual later effort envelopes are tested; this is bounded replay evidence, not arbitrary-input reachability.".into(),"Physiology clipping and course work are mechanical path evidence; empirical result plausibility still needs review.".into()]})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connected_default_chain_satisfies_declared_bounds_and_skipping_has_consequences() {
        let plan = compile(Options::default()).unwrap();
        let config = plan.config.clone();
        let pace = plan.pace_ms.clone();
        let report = plan.report().unwrap();
        assert!(
            report.acceptance.accepted,
            "{:?}",
            report.acceptance.reasons
        );
        let cue = &report.runs[0];
        let miss = report
            .runs
            .iter()
            .find(|r| r.execution == Execution::MissSecond)
            .unwrap();
        assert!(miss.steps[3].exit_deficit_m > cue.steps[3].exit_deficit_m + 100.0);
        assert_eq!(miss.steps[3].effort_share, 0.0);
        assert!(miss.finish_s.is_none());
        assert_eq!(plan.config, config);
        assert_eq!(plan.pace_ms, pace);
    }
    #[test]
    fn failed_bounded_replay_and_early_finish_reject_the_plan() {
        let plan = compile(Options::default()).unwrap();
        let mut report = plan.report().unwrap();
        report.runs[0].steps[2].effort_share = 0.5;
        assert!(!assess(&report.runs).workload);
        report.runs[0].steps[2].effort_share = 1.0;
        report.runs[0].finish_s = Some(200.0);
        assert!(!assess(&report.runs).finish_window);
        assert!(!assess(&report.runs).accepted);
        assert!(!assess(&[]).accepted);
    }
    #[test]
    fn actor_and_opponents_do_not_read_the_authored_effort_or_answer() {
        let mut plan = compile(Options::default()).unwrap();
        let before = replay(&plan, Execution::Cue).unwrap();
        for p in &mut plan.phases {
            p.intensity = 0.1;
            p.action = "unavailable answer".into();
            p.decisive = false;
        }
        let after = replay(&plan, Execution::Cue).unwrap();
        for (a, b) in before.trace.iter().zip(&after.trace) {
            assert_eq!(
                (a.distance_m, a.delivered_w, &a.view),
                (b.distance_m, b.delivered_w, &b.view)
            );
        }
        assert_eq!(before.finish_s, after.finish_s);
    }
    #[test]
    fn closest_other_file_rider_is_not_treated_as_a_sheltered_wheel() {
        let plan = compile(Options::default()).unwrap();
        let before = actor(
            &plan.config,
            &plan.initial,
            plan.learning.knowledge(),
            false,
        )
        .unwrap();
        let state = RaceState::new(
            plan.initial.simulation_time(),
            plan.initial.random_state(),
            plan.initial
                .riders()
                .iter()
                .map(|r| {
                    if r.id().as_str() == NAMES[1] {
                        RiderState::new(
                            r.id().clone(),
                            plan.initial.riders()[0].distance_m() + 2.0,
                            r.speed_ms(),
                            2,
                            None,
                        )
                        .unwrap()
                        .with_w_prime_balance(20000.0)
                        .unwrap()
                    } else {
                        r.clone()
                    }
                })
                .collect(),
        )
        .unwrap();
        let view = PublicRaceView::observe(&plan.config, &state, NAMES[0]).unwrap();
        assert_eq!(
            view.riders
                .iter()
                .find(|r| r.id == NAMES[1])
                .unwrap()
                .lateral_slot,
            2
        );
        let after = actor(&plan.config, &state, plan.learning.knowledge(), false).unwrap();
        assert_eq!(
            before, after,
            "A nearby other-file rider cannot change the selected sheltered wheel."
        );
    }
    #[test]
    fn course_selection_is_capacity_limited_without_an_archetype_veto() {
        let mut plan = compile(Options::default()).unwrap();
        let mut riders = plan.config.riders().to_vec();
        // Same identity, reputation and role; change only an explicit pre-ride capability fixture.
        let r = &riders[3];
        riders[3] = RiderDefinition::new(r.id().clone(), *r.physical())
            .unwrap()
            .with_physiology(race_physics::PhysiologyParams::new(120.0, 1000.0, 1100.0).unwrap());
        let c = &plan.config;
        plan.config = RaceSimulationConfig::new(RaceSimulationConfigSpec {
            fixed_step: c.fixed_step(),
            riders,
            terrain: c.terrain().clone(),
            dynamics: *c.dynamics(),
            drafting: c.drafting().cloned(),
            lateral_slot_count: c.lateral_slot_count() as i64,
            air_density_kg_m3: c.air_density_kg_m3(),
            headwind_ms: c.headwind_ms(),
        })
        .unwrap();
        plan.initial = RaceState::new(
            plan.initial.simulation_time(),
            plan.initial.random_state(),
            plan.initial
                .riders()
                .iter()
                .map(|r| {
                    if r.id().as_str() == NAMES[3] {
                        r.clone().with_w_prime_balance(1000.0).unwrap()
                    } else {
                        r.clone()
                    }
                })
                .collect(),
        )
        .unwrap();
        let report = plan.report().unwrap();
        assert!(!report.acceptance.leader_capacity);
        assert!(!report.acceptance.accepted);
        let run = &report.runs[0];
        assert!(
            run.shortfall_kj[3] > 1.0,
            "Weak sustained capability cannot execute the same climbing pace."
        );
        assert!(run.climbing_work_kj[3] > 0.0);
        assert!(
            run.finishers.iter().all(|f| f.rider != NAMES[3]),
            "Insufficient climbing capacity must affect the path to the line."
        );
    }
}
