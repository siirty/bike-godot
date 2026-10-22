//! Explicit game rules and workout lifecycle over the provisional race model.
//! Preparation/recovery integrate only existing player physiology; opponents run only in Race.
use crate::{
    mixed::{MixedError, MixedOptions, MixedSession, Sample, PLAYER, RIDERS},
    RaceState, RiderId,
};
use race_physics::step_physiology;
use serde::Serialize;
use std::time::Duration;
use workout_import::WorkoutTimeline;

/// Fixed stepping matches the competitive kernel.
pub const STEP: Duration = Duration::from_millis(50);
/// Competitive start in the unchanged workout timeline.
pub const RACE_START: Duration = Duration::from_secs(600);
/// Competitive deadline; results freeze before recovery.
pub const RACE_END: Duration = Duration::from_secs(3300);
/// Complete workout duration.
pub const WORKOUT_END: Duration = Duration::from_secs(3600);
/// Explicit lifecycle, distinct from observed race topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Phase {
    /// Stationary trainer beside the bus; no competitive field yet.
    WarmUp,
    /// Only this phase advances the competitive kernel.
    Race,
    /// Stationary trainer; competitive state/result are immutable.
    CoolDown,
    /// No further work is recorded or advanced.
    Finished,
}
/// Workout completion is separate from the competitive outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Completion {
    /// Still riding or paused.
    InProgress,
    /// All workout steps were executed, even if some powers missed their targets.
    Completed,
    /// Workout time was explicitly skipped; recovery may still be in progress.
    Skipped,
    /// User ended the workout before its natural completion.
    Abandoned,
}
/// Provisional game verdict, never a world-model validity or learning claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    /// At least one Ember rider, including the player, is in the leading group at the deadline.
    Success,
    /// No Ember rider is in that group at the deadline.
    Failure,
    /// Competition was not played to the deadline.
    Incomplete,
}
/// Why the competitive result was captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EndReason {
    /// Natural competitive deadline.
    Deadline,
    /// User selected recovery early.
    SkipToCooldown,
    /// User ended the workout early.
    FinishEarly,
}
/// Frozen competitive evidence, outside kernel snapshots/events.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outcome {
    verdict: Verdict,
    reason: EndReason,
    workout_s: f64,
    race_played_s: f64,
    leading_group: Vec<String>,
    represented: Vec<String>,
    player_front_gap_m: Option<f64>,
}
impl Outcome {
    /// Game result or incomplete marker.
    pub fn verdict(&self) -> Verdict {
        self.verdict
    }
    /// Natural end versus explicit interruption.
    pub fn reason(&self) -> EndReason {
        self.reason
    }
    /// Eligible team representatives at the captured state; empty if competition never started.
    pub fn represented(&self) -> &[String] {
        &self.represented
    }
    /// Player's physical deficit when captured.
    pub fn player_front_gap_m(&self) -> Option<f64> {
        self.player_front_gap_m
    }
}
/// Lifecycle audit event; never a kernel RaceEvent or measured telemetry.
#[derive(Debug, Clone, Serialize)]
pub struct SessionEvent {
    /// Executed active time, excluding pauses and skipped workout duration.
    pub active_us: u64,
    /// Position in the prescribed workout, which may jump on explicit skip.
    pub workout_us: u64,
    /// Plain game rule/phase/control event.
    pub kind: String,
}
/// End-of-step software evidence, with the phase/block that produced this step.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GameSample {
    /// Actual executed time, not elapsed wall time.
    pub active_s: f64,
    /// Workout cursor after this step.
    pub workout_s: f64,
    /// Phase during this step; the live phase may have just transitioned.
    pub phase: Phase,
    /// Source workout block.
    pub block: usize,
    /// Nominal target.
    pub target_w: f64,
    /// Actual model-limited delivered software power.
    pub delivered_w: f64,
    /// Player reserve carried between phases.
    pub reserve_j: f64,
    /// Provisional ±5% FTP diagnostic.
    pub in_band: bool,
    /// Competitive geometry exists only for competitive steps.
    pub race: Option<Sample>,
}
/// Explicit, pure game lifecycle; Godot supplies time and controls outside this type.
#[derive(Debug, Clone)]
pub struct GameSession {
    template: MixedSession,
    race: Option<MixedSession>,
    timeline: WorkoutTimeline,
    phase: Phase,
    active: Duration,
    workout: Duration,
    reserve: f64,
    paused: bool,
    completion: Completion,
    outcome: Option<Outcome>,
    last: Option<GameSample>,
    events: Vec<SessionEvent>,
}
impl GameSession {
    /// Prepare a fresh race-only plan; NPCs are explicitly fresh at their initial grid.
    pub fn new(options: MixedOptions) -> Result<Self, MixedError> {
        Self::from_template(MixedSession::race_template(options)?)
    }
    /// Reuse a frozen competitive template, without repeating the offline fit.
    pub fn from_template(template: MixedSession) -> Result<Self, MixedError> {
        if template.state().simulation_time() != RACE_START || template.last_sample().is_some() {
            return Err(MixedError::Invalid(
                "fresh competitive-only template required",
            ));
        }
        Ok(Self {
            template,
            race: None,
            timeline: WorkoutTimeline::new(session_director::mixed::workout()).unwrap(),
            phase: Phase::WarmUp,
            active: Duration::ZERO,
            workout: Duration::ZERO,
            reserve: 20000.0,
            paused: false,
            completion: Completion::InProgress,
            outcome: None,
            last: None,
            events: vec![],
        })
    }
    /// Frozen plan/config identity used by replay validation and rendering.
    pub fn template(&self) -> &MixedSession {
        &self.template
    }
    /// Current lifecycle phase.
    pub fn phase(&self) -> Phase {
        self.phase
    }
    /// True only when explicit pause is active.
    pub fn paused(&self) -> bool {
        self.paused
    }
    /// Executed time, unaffected by skips and pause wall time.
    pub fn active_time(&self) -> Duration {
        self.active
    }
    /// Source workout cursor.
    pub fn workout_time(&self) -> Duration {
        self.workout
    }
    /// Player model reserve; not an empirically validated fatigue measurement.
    pub fn reserve_j(&self) -> f64 {
        self.reserve
    }
    /// Workout execution status, independent of race success.
    pub fn completion(&self) -> Completion {
        self.completion
    }
    /// Immutable competitive result, if it has ended or been interrupted.
    pub fn outcome(&self) -> Option<&Outcome> {
        self.outcome.as_ref()
    }
    /// Last actual fixed step; commands do not manufacture samples.
    pub fn last_sample(&self) -> Option<&GameSample> {
        self.last.as_ref()
    }
    /// Actual competitive state, absent during preparation and frozen during recovery.
    pub fn race_state(&self) -> Option<&RaceState> {
        self.race.as_ref().map(MixedSession::state)
    }
    /// Application phase/control audit, not a kernel event stream.
    pub fn events(&self) -> &[SessionEvent] {
        &self.events
    }
    /// Current target is a software reference only, never trainer control.
    pub fn target_w(&self) -> f64 {
        self.timeline
            .power_at(self.workout, self.template.options().0)
            .unwrap()
            .unwrap()
    }
    /// Half-open source block.
    pub fn block(&self) -> usize {
        [600, 1500, 1680, 2280, 2640, 2700, 3300]
            .into_iter()
            .filter(|s| self.workout.as_secs() >= *s)
            .count()
    }
    fn event(&mut self, kind: &str) {
        self.events.push(SessionEvent {
            active_us: self.active.as_micros() as u64,
            workout_us: self.workout.as_micros() as u64,
            kind: kind.into(),
        });
    }
    /// Pause preserves all physical and workout state; repeated same-state requests are no-ops.
    pub fn set_paused(&mut self, paused: bool) -> bool {
        if self.phase == Phase::Finished || self.paused == paused {
            return false;
        }
        self.paused = paused;
        self.event(if paused { "Paused" } else { "Resumed" });
        true
    }
    fn capture(&mut self, reason: EndReason) {
        if self.outcome.is_some() {
            return;
        }
        let leading_group = self
            .race
            .as_ref()
            .and_then(|r| r.observation().groups().first())
            .map(|g| g.members().to_vec())
            .unwrap_or_default();
        let knowledge = session_director::mixed::knowledge();
        let represented: Vec<_> = leading_group
            .iter()
            .filter(|r| knowledge.teammates(PLAYER, r))
            .cloned()
            .collect();
        let player_front_gap_m = self.race_state().map(|s| {
            let front = s
                .riders()
                .iter()
                .map(|r| r.distance_m())
                .max_by(f64::total_cmp)
                .unwrap();
            front
                - s.rider_state(&RiderId::new(PLAYER).unwrap())
                    .unwrap()
                    .distance_m()
        });
        let verdict = if reason != EndReason::Deadline {
            Verdict::Incomplete
        } else if represented.is_empty() {
            Verdict::Failure
        } else {
            Verdict::Success
        };
        self.outcome = Some(Outcome {
            verdict,
            reason,
            workout_s: self.workout.as_secs_f64(),
            race_played_s: self.workout.saturating_sub(RACE_START).as_secs_f64(),
            leading_group,
            represented,
            player_front_gap_m,
        });
        self.event("CompetitiveResultFrozen");
    }
    /// Enter recovery early; the skipped race cannot count as a successful completion.
    pub fn skip_to_cooldown(&mut self) -> bool {
        if !matches!(self.phase, Phase::WarmUp | Phase::Race) {
            return false;
        }
        self.capture(EndReason::SkipToCooldown);
        self.completion = Completion::Skipped;
        self.workout = RACE_END;
        self.phase = Phase::CoolDown;
        self.event("SkippedToCooldown");
        true
    }
    /// End immediately, preserving any already resolved competitive outcome.
    pub fn finish(&mut self) -> bool {
        if self.phase == Phase::Finished {
            return false;
        }
        self.capture(EndReason::FinishEarly);
        self.completion = Completion::Abandoned;
        self.phase = Phase::Finished;
        self.paused = false;
        self.event("FinishedEarly");
        true
    }
    /// Execute one 50 ms software step. Preparation/recovery never step or reposition opponents.
    pub fn step(
        &mut self,
        watts: f64,
        slot: usize,
        wheel: Option<&str>,
    ) -> Result<GameSample, MixedError> {
        if self.phase == Phase::Finished || self.paused {
            return Err(MixedError::Invalid("session is finished or paused"));
        }
        if !watts.is_finite()
            || watts < 0.0
            || slot >= 3
            || wheel.is_some_and(|w| w == PLAYER || !RIDERS.contains(&w))
        {
            return Err(MixedError::Invalid("invalid game controls"));
        }
        let phase = self.phase;
        let block = self.block();
        let target_w = self.target_w();
        let (delivered_w, race) = if phase == Phase::Race {
            let step = self.race.as_mut().unwrap().step(watts, slot, wheel)?;
            self.reserve = step
                .sample
                .riders
                .iter()
                .find(|r| r.id == PLAYER)
                .unwrap()
                .reserve_j;
            (step.sample.delivered_w, Some(step.sample))
        } else {
            let cap = self
                .template
                .config()
                .definition_for(&RiderId::new(PLAYER).unwrap())
                .unwrap()
                .physiology()
                .unwrap();
            let result = step_physiology(cap, self.reserve, watts, STEP)
                .map_err(|_| MixedError::Invalid("invalid player physiology input"))?;
            self.reserve = result.balance_j;
            (result.delivered_power_w, None)
        };
        self.active += STEP;
        self.workout += STEP;
        let sample = GameSample {
            active_s: self.active.as_secs_f64(),
            workout_s: self.workout.as_secs_f64(),
            phase,
            block,
            target_w,
            delivered_w,
            reserve_j: self.reserve,
            in_band: (delivered_w - target_w).abs() <= 0.05 * self.template.options().0,
            race,
        };
        if self.workout == RACE_START {
            self.race = Some(self.template.clone().with_starting_reserve(self.reserve)?);
            self.phase = Phase::Race;
            self.event("RaceStarted");
        } else if self.workout == RACE_END {
            self.capture(EndReason::Deadline);
            self.phase = Phase::CoolDown;
            self.event("CooldownStarted");
        } else if self.workout == WORKOUT_END {
            self.phase = Phase::Finished;
            if self.completion == Completion::InProgress {
                self.completion = Completion::Completed;
            }
            self.event("WorkoutEnded");
        }
        self.last = Some(sample.clone());
        Ok(sample)
    }
}
