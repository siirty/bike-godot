//! The full ride loop against a real `SimulatedTrainer` and a recording trainer, with literal
//! times instead of wall-clock waits. Ported from the prototype's ride-session suites.

use std::time::Duration;

use race_engine::{RideFrame, RideSession, RideSessionError, DEFAULT_RENDER_INTERVAL};
use race_physics::{solve_speed_for_power, EnvironmentParams, RiderPhysicalParams, SolverOptions};
use session_director::{FlatTerrain, RiderProfile, TerrainSource};
use trainer_control::{
    RecordingTrainerControl, SimulatedTrainer, SimulatedTrainerOptions, TrainerControl,
    TrainerTelemetry,
};
use workout_import::{PowerTarget, WorkoutPlan, WorkoutSegment};

const MS: fn(u64) -> Duration = Duration::from_millis;
const S: fn(u64) -> Duration = Duration::from_secs;

fn steady(seconds: u64, watts: f64) -> WorkoutSegment {
    WorkoutSegment::SteadyState {
        duration: S(seconds),
        target_power: PowerTarget::AbsoluteWatts(watts),
        cadence: None,
    }
}

fn plan(segments: Vec<WorkoutSegment>) -> WorkoutPlan {
    WorkoutPlan {
        segments,
        ..WorkoutPlan::default()
    }
}

fn rider() -> RiderPhysicalParams {
    RiderPhysicalParams::new(80.0)
}

fn sim_trainer(report_ms: u64, report_elapsed_time: bool) -> SimulatedTrainer {
    SimulatedTrainer::new(
        rider(),
        SimulatedTrainerOptions {
            tick_interval: MS(50),
            report_interval: MS(report_ms),
            report_elapsed_time,
            ..SimulatedTrainerOptions::default()
        },
    )
}

fn session<T: TrainerControl>(
    plan: WorkoutPlan,
    trainer: T,
    terrain: Box<dyn TerrainSource>,
) -> RideSession<T> {
    RideSession::new(
        plan,
        RiderProfile::new(200.0, rider()),
        trainer,
        terrain,
        MS(50),
    )
    .unwrap()
}

fn set_power<T: TrainerControl>(s: &mut RideSession<T>, watts: f64) {
    s.trainer_mut()
        .manual_power_source()
        .expect("simulated trainer")
        .set_rider_power_watts(watts);
}

fn steady_speed(watts: f64) -> f64 {
    solve_speed_for_power(
        watts,
        &rider(),
        &EnvironmentParams::default(),
        SolverOptions::default(),
    )
    .unwrap()
}

fn advance<T: TrainerControl>(s: &mut RideSession<T>, now: Duration, frames: &mut Vec<RideFrame>) {
    frames.extend(s.advance(now).frames);
}

#[test]
fn tracks_interval_progression_against_a_simulated_trainer() {
    let segments = vec![steady(10, 200.0), steady(5, 100.0)];
    let mut s = session(
        plan(segments.clone()),
        sim_trainer(100, true),
        Box::new(FlatTerrain::default()),
    );
    let mut frames = Vec::new();
    s.start(S(0)).unwrap();
    set_power(&mut s, 200.0);

    advance(&mut s, S(5), &mut frames);
    let last = frames.last().unwrap();
    assert_eq!(last.current_segment, segments[0]);
    assert_eq!(last.segment_index, 0);
    assert_eq!(last.target_power_watts, Some(200.0));
    assert!(!last.ride_complete);
    assert!(last.time_remaining_in_segment.abs_diff(S(5)) <= MS(200));

    advance(&mut s, S(10), &mut frames);
    let last = frames.last().unwrap();
    assert_eq!(last.current_segment, segments[1]);
    assert_eq!(last.target_power_watts, Some(100.0));
    assert!(!last.ride_complete);

    set_power(&mut s, 100.0);
    advance(&mut s, S(15), &mut frames);
    let last = frames.last().unwrap();
    assert!(last.ride_complete);
    assert_eq!(last.time_remaining_in_segment, Duration::ZERO);
    assert!((last.speed_ms - steady_speed(100.0)).abs() < 0.5);
}

#[test]
fn emits_nothing_after_the_complete_frame() {
    let mut s = session(
        plan(vec![steady(2, 150.0)]),
        sim_trainer(100, true),
        Box::new(FlatTerrain::default()),
    );
    let mut frames = Vec::new();
    s.start(S(0)).unwrap();
    set_power(&mut s, 150.0);
    advance(&mut s, S(3), &mut frames);
    assert!(frames.last().unwrap().ride_complete);
    assert_eq!(frames.iter().filter(|f| f.ride_complete).count(), 1);
    assert!(s.is_complete());
    let distance = s.last_distance_m();
    assert!(s.advance(S(10)).frames.is_empty());
    assert_eq!(s.last_distance_m(), distance);
}

#[test]
fn completes_on_a_trainer_that_never_reports_elapsed_time() {
    let mut s = session(
        plan(vec![steady(2, 150.0)]),
        sim_trainer(100, false),
        Box::new(FlatTerrain::default()),
    );
    let mut frames = Vec::new();
    s.start(S(0)).unwrap();
    set_power(&mut s, 150.0);

    advance(&mut s, S(1), &mut frames);
    assert!(frames.last().unwrap().elapsed.abs_diff(S(1)) <= MS(100));

    advance(&mut s, S(3), &mut frames);
    let last = frames.last().unwrap();
    assert!(last.ride_complete);
    assert_eq!(last.elapsed, S(2));
    assert!((last.distance_m - steady_speed(150.0) * 2.0).abs() < 1.0);
}

#[test]
fn ticks_fall_on_the_render_grid_however_the_caller_slices_time() {
    let run = |slices: &[u64]| {
        let mut s = session(
            plan(vec![steady(3, 180.0)]),
            sim_trainer(250, true),
            Box::new(FlatTerrain::default()),
        );
        s.start(S(0)).unwrap();
        set_power(&mut s, 180.0);
        let mut frames = Vec::new();
        for &ms in slices {
            advance(&mut s, MS(ms), &mut frames);
        }
        frames
    };
    let coarse = run(&[3000]);
    let fine: Vec<u64> = (1..=300).map(|k| k * 10).collect();
    assert_eq!(coarse, run(&fine));
    // One frame per 50 ms, and on a trainer whose clock agrees, each frame's elapsed is its tick.
    assert_eq!(coarse.len(), 60);
    for (k, frame) in coarse.iter().enumerate() {
        assert_eq!(frame.elapsed, MS(50 * (k as u64 + 1)));
    }
}

#[test]
fn a_second_start_is_an_error_not_a_second_trainer_session() {
    let mut s = session(
        plan(vec![steady(2, 150.0)]),
        RecordingTrainerControl::new(),
        Box::new(FlatTerrain::default()),
    );
    s.start(S(0)).unwrap();
    assert_eq!(s.start(S(1)), Err(RideSessionError::AlreadyStarted));
    assert_eq!(s.trainer().inner().connect_count, 1);
    assert_eq!(s.trainer().inner().start_count, 1);
}

#[test]
fn a_connect_failure_reaches_the_caller() {
    let mut trainer = RecordingTrainerControl::new();
    trainer.connect_error = Some("no adapter".into());
    let mut s = session(
        plan(vec![steady(2, 150.0)]),
        trainer,
        Box::new(FlatTerrain::default()),
    );
    assert!(matches!(s.start(S(0)), Err(RideSessionError::Trainer(_))));
}

#[test]
fn an_empty_plan_is_rejected_at_construction() {
    let r = RideSession::new(
        plan(vec![]),
        RiderProfile::new(200.0, rider()),
        RecordingTrainerControl::new(),
        Box::new(FlatTerrain::default()),
        DEFAULT_RENDER_INTERVAL,
    );
    assert!(matches!(r, Err(RideSessionError::InvalidPlan(_))));
}

#[test]
fn completion_leaves_the_injected_trainer_running() {
    let mut s = session(
        plan(vec![steady(1, 150.0)]),
        RecordingTrainerControl::new(),
        Box::new(FlatTerrain::default()),
    );
    s.start(S(0)).unwrap();
    s.advance(S(2));
    assert!(s.is_complete());
    let trainer = s.into_trainer();
    assert_eq!((trainer.stop_count, trainer.dispose_count), (0, 0));
}

struct Step {
    at_m: f64,
    before: f64,
    after: f64,
}

impl TerrainSource for Step {
    fn grade_at_distance(&self, d: f64) -> f64 {
        if d < self.at_m {
            self.before
        } else {
            self.after
        }
    }
    fn length_m(&self) -> Option<f64> {
        None
    }
}

#[test]
fn writes_the_starting_grade_then_follows_the_terrain_through_the_throttle() {
    let terrain = Step {
        at_m: 50.0,
        before: 0.08,
        after: 0.02,
    };
    let mut s = session(
        plan(vec![steady(30, 150.0)]),
        RecordingTrainerControl::new(),
        Box::new(terrain),
    );
    s.start(S(0)).unwrap();
    assert_eq!(s.trainer().inner().grades_written(), vec![0.08]);

    // The rider reports 10 m/s from the start: 50 m after 5 s.
    for k in 1..=200u64 {
        let elapsed = MS(k * 100);
        // Deliver each report at its own instant: a recorder hands over whatever is queued.
        s.advance(elapsed - MS(1));
        s.trainer_mut()
            .inner_mut()
            .emit_telemetry(TrainerTelemetry {
                speed_ms: 10.0,
                power_watts: 150.0,
                cadence_rpm: Some(90.0),
                total_distance_m: elapsed.as_secs_f64() * 10.0,
                elapsed_session_time: Some(elapsed),
            });
        s.advance(elapsed);
    }
    let grades = s.trainer().inner().grades_written();
    assert_eq!(
        grades,
        vec![0.08, 0.02],
        "one write per material change, not one per tick"
    );
    let (_, at) = s.trainer().inner().sim_param_writes[1];
    assert_eq!(
        at,
        MS(5000),
        "the grade changes on the tick the rider reaches 50 m"
    );
}
