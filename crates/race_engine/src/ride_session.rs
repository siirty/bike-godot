//! The live single-rider ride loop, driven by explicit time.
//!
//! The prototype ran this on `Timer.periodic` plus a telemetry stream. Here the caller (the Godot
//! frame loop, or a test) calls [`RideSession::advance`] with a monotonic `now`, and the session
//! replays every render tick due up to `now`, polling the trainer at each tick's own time. The
//! results are the same as the timer version, and tests use literal times instead of fake async.
//!
//! What the session does: integrate the human's position from trainer telemetry, hold the last
//! *reported* speed between the trainer's low-rate reports (never re-deriving it), track the
//! workout timeline, and write the grade at the rider's current distance back through
//! [`ThrottledTrainerControl`]. It decides nothing about terrain or pacing.

use std::time::Duration;

use race_physics::EnvironmentParams;
use session_director::{RiderProfile, TerrainSource};
use thiserror::Error;
use trainer_control::{
    SimulationParameters, ThrottledTrainerControl, TrainerConnectionState, TrainerControl,
    TrainerError, TrainerEvent, TrainerTelemetry,
};
use workout_import::{WorkoutImportError, WorkoutPlan, WorkoutSegment, WorkoutTimeline};

/// The default render cadence, about 30 fps. Independent of the trainer's report rate and its
/// internal physics tick.
pub const DEFAULT_RENDER_INTERVAL: Duration = Duration::from_millis(33);

/// One rendered moment of a live ride: plain data with no rendering dependency.
#[derive(Debug, Clone, PartialEq)]
pub struct RideFrame {
    /// Ride time since start, clamped to the plan's length.
    pub elapsed: Duration,
    /// Last reported ground speed, m/s.
    pub speed_ms: f64,
    /// Last reported power, W.
    pub power_watts: f64,
    /// Distance this ride, m: last report plus extrapolation at the reported speed.
    pub distance_m: f64,
    /// The plan's target at `elapsed`; `None` in a free ride.
    pub target_power_watts: Option<f64>,
    /// Index of the current segment in the plan.
    pub segment_index: usize,
    /// The current segment.
    pub current_segment: WorkoutSegment,
    /// Time left in the current segment.
    pub time_remaining_in_segment: Duration,
    /// Whether this frame is the end of the plan. It is the last frame.
    pub ride_complete: bool,
    /// Last reported cadence; `None` when the trainer does not report it.
    pub cadence_rpm: Option<f64>,
}

/// Why a session could not start.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RideSessionError {
    /// The plan has no duration.
    #[error(transparent)]
    InvalidPlan(#[from] WorkoutImportError),
    /// `start` was called twice.
    #[error("RideSession::start has already been called")]
    AlreadyStarted,
    /// The trainer refused to connect, take the first parameters, or start.
    #[error(transparent)]
    Trainer(#[from] TrainerError),
}

/// What happened during one [`RideSession::advance`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RideAdvance {
    /// One frame per render tick that fell due, in order. The last one may be complete.
    pub frames: Vec<RideFrame>,
    /// Connection transitions the trainer reported, in order.
    pub connection_states: Vec<TrainerConnectionState>,
    /// A grade write that failed. Ride frames keep coming; the next material change retries.
    pub write_errors: Vec<TrainerError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Running,
    Complete,
}

/// The live ride loop over one trainer.
pub struct RideSession<T: TrainerControl> {
    timeline: WorkoutTimeline,
    rider: RiderProfile,
    trainer: ThrottledTrainerControl<T>,
    terrain: Box<dyn TerrainSource>,
    render_interval: Duration,
    phase: Phase,
    /// Monotonic time of `start`; render tick `k` falls at `started_at + k * render_interval`.
    started_at: Duration,
    ticks_rendered: u32,

    last_reported_elapsed: Duration,
    last_reported_distance_m: f64,
    last_reported_power_watts: f64,
    last_reported_speed_ms: f64,
    last_reported_cadence_rpm: Option<f64>,
    render_elapsed: Duration,
    last_interpolated_distance_m: f64,
}

impl<T: TrainerControl> RideSession<T> {
    /// A session over `trainer`, which it wraps in the write throttle. An empty plan is rejected
    /// here, where the caller can see it, rather than from inside a tick.
    pub fn new(
        plan: WorkoutPlan,
        rider: RiderProfile,
        trainer: T,
        terrain: Box<dyn TerrainSource>,
        render_interval: Duration,
    ) -> Result<Self, RideSessionError> {
        assert!(
            !render_interval.is_zero(),
            "render interval must be positive"
        );
        Ok(Self {
            timeline: WorkoutTimeline::new(plan)?,
            rider,
            trainer: ThrottledTrainerControl::new(trainer),
            terrain,
            render_interval,
            phase: Phase::Idle,
            started_at: Duration::ZERO,
            ticks_rendered: 0,
            last_reported_elapsed: Duration::ZERO,
            last_reported_distance_m: 0.0,
            last_reported_power_watts: 0.0,
            last_reported_speed_ms: 0.0,
            last_reported_cadence_rpm: None,
            render_elapsed: Duration::ZERO,
            last_interpolated_distance_m: 0.0,
        })
    }

    /// Connects, writes the grade at distance 0, and starts the trainer.
    pub fn start(&mut self, now: Duration) -> Result<(), RideSessionError> {
        if self.phase != Phase::Idle {
            return Err(RideSessionError::AlreadyStarted);
        }
        // Marked before the fallible calls, so a failed start cannot be retried into a second
        // set of trainer calls on a half-started trainer.
        self.phase = Phase::Running;
        self.started_at = now;
        self.trainer.connect(now)?;
        let params = self.sim_params_for_distance(0.0);
        self.trainer.set_simulation_parameters(params, now)?;
        self.trainer.start(now)?;
        Ok(())
    }

    /// Replays every render tick due up to `now`. Each tick first polls the trainer at that tick's
    /// time, so a report and a tick at the same instant see the report first. Returns nothing
    /// before `start` and after the complete frame.
    pub fn advance(&mut self, now: Duration) -> RideAdvance {
        let mut out = RideAdvance::default();
        while self.phase == Phase::Running {
            let tick_at = self.started_at + self.render_interval * (self.ticks_rendered + 1);
            if tick_at > now {
                break;
            }
            self.ticks_rendered += 1;
            // The ride clock moves to this tick first, then the reports due by now re-anchor it.
            // (The prototype advanced after the reports, so every snap put the clock one render
            // interval ahead of the trainer's own elapsed time.)
            self.render_elapsed += self.render_interval;
            for event in self.trainer.poll(tick_at) {
                match event {
                    TrainerEvent::Telemetry(t) => self.on_telemetry(&t),
                    TrainerEvent::ConnectionState(s) => out.connection_states.push(s),
                }
            }
            let frame = self.render_tick();
            let complete = frame.ride_complete;
            out.frames.push(frame);
            if complete {
                // The trainer is not stopped: it was injected, and its owner decides.
                self.phase = Phase::Complete;
            } else {
                let params = self.sim_params_for_distance(self.last_interpolated_distance_m);
                if let Err(e) = self.trainer.set_simulation_parameters(params, tick_at) {
                    out.write_errors.push(e);
                }
            }
        }
        out
    }

    /// Whether the complete frame has been emitted.
    pub fn is_complete(&self) -> bool {
        self.phase == Phase::Complete
    }

    /// The distance the last frame reported, m.
    pub fn last_distance_m(&self) -> f64 {
        self.last_interpolated_distance_m
    }

    /// The plan's total duration.
    pub fn total_duration(&self) -> Duration {
        self.timeline.total_duration()
    }

    /// The (throttled) trainer, for its connection state or manual-power capability.
    pub fn trainer_mut(&mut self) -> &mut ThrottledTrainerControl<T> {
        &mut self.trainer
    }

    /// The (throttled) trainer.
    pub fn trainer(&self) -> &ThrottledTrainerControl<T> {
        &self.trainer
    }

    /// Ends the session and hands the trainer back to its owner, unstopped and undisposed.
    pub fn into_trainer(self) -> T {
        self.trainer.into_inner()
    }

    fn on_telemetry(&mut self, t: &TrainerTelemetry) {
        self.last_reported_distance_m = t.total_distance_m;
        self.last_reported_power_watts = t.power_watts;
        self.last_reported_speed_ms = t.speed_ms;
        self.last_reported_cadence_rpm = t.cadence_rpm;
        // FTMS Elapsed Time is optional. When present, snap the render clock to it so drift never
        // accumulates; when absent, the render clock *is* the ride clock, and only the
        // extrapolation anchor moves to now.
        match t.elapsed_session_time {
            Some(reported) => {
                self.last_reported_elapsed = reported;
                self.render_elapsed = reported;
            }
            None => self.last_reported_elapsed = self.render_elapsed,
        }
    }

    fn render_tick(&mut self) -> RideFrame {
        let total = self.timeline.total_duration();
        let elapsed = self.render_elapsed.min(total);
        // Signed: a report can carry an elapsed time slightly ahead of the render clock.
        let dt_seconds = (elapsed.as_micros() as i64
            - self.last_reported_elapsed.as_micros() as i64) as f64
            / 1_000_000.0;
        let distance_m = self.last_reported_distance_m + self.last_reported_speed_ms * dt_seconds;
        self.last_interpolated_distance_m = distance_m;
        let in_range = "elapsed is clamped to the plan";
        let segment_index = self.timeline.index_at(elapsed).expect(in_range);
        RideFrame {
            elapsed,
            speed_ms: self.last_reported_speed_ms,
            power_watts: self.last_reported_power_watts,
            distance_m,
            target_power_watts: self
                .timeline
                .power_at(elapsed, self.rider.ftp_watts)
                .expect(in_range),
            segment_index,
            current_segment: self.timeline.plan().segments[segment_index].clone(),
            time_remaining_in_segment: self.timeline.time_remaining_at(elapsed).expect(in_range),
            ride_complete: elapsed >= total,
            cadence_rpm: self.last_reported_cadence_rpm,
        }
    }

    fn sim_params_for_distance(&self, distance_m: f64) -> SimulationParameters {
        SimulationParameters::from_physics(
            &self.rider.physical,
            &EnvironmentParams::with_grade(self.terrain.grade_at_distance(distance_m)),
        )
    }
}
