//! Game session controls, presentation and versioned software replay. No trainer/history access.
use crate::app::{format::format_duration, race::RaceRider, route::elevation_table};
use race_engine::{
    encode_snapshot,
    game_session::*,
    mixed::{MixedOptions, PLAYER, RIDERS},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{sync::OnceLock, time::Duration};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Action {
    Input {
        watts: f64,
        slot: usize,
        wheel: Option<String>,
    },
    Pause {
        value: bool,
    },
    SkipToCooldown,
    Finish,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    at_us: u64,
    action: Action,
}
#[derive(Deserialize)]
struct ReplayFile {
    schema_version: u32,
    policy: String,
    software_only: bool,
    initial_kernel_snapshot: String,
    end_us: u64,
    commands: Vec<Entry>,
}
struct Replay {
    end: Duration,
    commands: Vec<Entry>,
    index: usize,
    input: Option<(f64, usize, Option<String>)>,
}
/// Plain Rust presentation. The engine model is provisional; game rules are explicit.
pub struct MixedView {
    /// Essential objective/roster, never gated by coaching.
    pub context: String,
    /// Workout cursor and executed active time.
    pub clock: String,
    /// Current phase label.
    pub phase: String,
    /// Public phase instruction.
    pub instruction: String,
    /// Frozen game result plus workout completion status.
    pub result: String,
    /// Private model diagnostics, shown only after explicit reveal.
    pub diagnostics: String,
    /// Last control/export/replay status.
    pub message: String,
    /// Whether the session stopped; this does not imply successful completion.
    pub complete: bool,
    /// Explicit pause state.
    pub paused: bool,
    /// Show stationary bus scene rather than competitive field.
    pub at_bus: bool,
    /// Renderable game riders; only the player is shown at the bus.
    pub riders: Vec<RaceRider>,
}
/// UI-independent game adapter, using the same lifecycle as the headless report.
pub struct MixedRace {
    initial: GameSession,
    session: GameSession,
    accumulator: f64,
    manual: Option<f64>,
    slot: usize,
    wheel: Option<String>,
    samples: Vec<GameSample>,
    commands: Vec<Entry>,
    last_input: Option<(f64, usize, Option<String>)>,
    replay: Option<Replay>,
    replay_hold: bool,
    message: String,
    steps: u32,
    in_band: u32,
}
impl Default for MixedRace {
    fn default() -> Self {
        Self::new()
    }
}
impl MixedRace {
    /// Compile the competitive-only plan once; reopening/restarting clones frozen data.
    pub fn new() -> Self {
        static INITIAL: OnceLock<GameSession> = OnceLock::new();
        let initial = INITIAL
            .get_or_init(|| GameSession::new(MixedOptions::default()).expect("valid game fixture"))
            .clone();
        Self {
            session: initial.clone(),
            initial,
            accumulator: 0.0,
            manual: None,
            slot: 1,
            wheel: None,
            samples: vec![],
            commands: vec![],
            last_input: None,
            replay: None,
            replay_hold: false,
            message: String::new(),
            steps: 0,
            in_band: 0,
        }
    }
    /// Clear all lifecycle/policy/audit state; retain visible manual/lane/wheel selections.
    pub fn reset(&mut self) {
        self.session = self.initial.clone();
        self.accumulator = 0.0;
        self.samples.clear();
        self.commands.clear();
        self.last_input = None;
        self.replay = None;
        self.replay_hold = false;
        self.message.clear();
        self.steps = 0;
        self.in_band = 0;
    }
    fn editable(&mut self) -> bool {
        if self.replay.is_some() || self.session.phase() == Phase::Finished {
            self.message = "Restart before changing replay or finished-session controls.".into();
            false
        } else {
            true
        }
    }
    /// Software power only; None selects nominal reference input.
    pub fn set_power(&mut self, power: Option<f64>) -> bool {
        if !self.editable() {
            return false;
        }
        if power.is_some_and(|w| !w.is_finite() || !(0.0..=1800.0).contains(&w)) {
            self.message = "Choose 0–1800 W.".into();
            return false;
        }
        self.manual = power;
        self.message.clear();
        true
    }
    /// Request a valid road slot; it has no competitive effect on the stationary trainer.
    pub fn set_slot(&mut self, slot: i64) -> bool {
        if !self.editable() || !(0..3).contains(&slot) {
            return false;
        }
        self.slot = slot as usize;
        true
    }
    /// Request a known wheel. Eligibility is still resolved only by the race kernel.
    pub fn set_wheel(&mut self, name: &str) -> bool {
        if !self.editable() || (!name.is_empty() && (name == PLAYER || !RIDERS.contains(&name))) {
            return false;
        }
        self.wheel = (!name.is_empty()).then(|| name.into());
        true
    }
    fn record(&mut self, action: Action) {
        self.commands.push(Entry {
            at_us: self.session.active_time().as_micros() as u64,
            action,
        });
    }
    /// Pause is audited and holds all clocks/state. Replay omits the wall-time wait.
    pub fn set_paused(&mut self, value: bool) -> bool {
        if self.replay.is_some() {
            if self.replay_hold == value {
                return false;
            }
            self.replay_hold = value;
            self.accumulator = 0.0;
            return true;
        }
        if !self.editable() || !self.session.set_paused(value) {
            return false;
        }
        self.accumulator = 0.0;
        self.record(Action::Pause { value });
        true
    }
    /// Capture an incomplete race and jump the workout cursor to recovery.
    pub fn skip_to_cooldown(&mut self) -> bool {
        if !self.editable() || !self.session.skip_to_cooldown() {
            return false;
        }
        self.accumulator = 0.0;
        self.record(Action::SkipToCooldown);
        true
    }
    /// End now, keeping an already resolved race result while marking the workout abandoned.
    pub fn finish(&mut self) -> bool {
        if !self.editable() || !self.session.finish() {
            return false;
        }
        self.accumulator = 0.0;
        self.record(Action::Finish);
        true
    }
    fn pump_replay(&mut self) {
        loop {
            let entry = self
                .replay
                .as_ref()
                .and_then(|r| r.commands.get(r.index))
                .filter(|e| e.at_us == self.session.active_time().as_micros() as u64)
                .cloned();
            let Some(entry) = entry else {
                break;
            };
            match &entry.action {
                Action::Input { watts, slot, wheel } => {
                    self.replay.as_mut().unwrap().input = Some((*watts, *slot, wheel.clone()))
                }
                Action::Pause { value } => {
                    self.session.set_paused(*value);
                }
                Action::SkipToCooldown => {
                    self.session.skip_to_cooldown();
                }
                Action::Finish => {
                    self.session.finish();
                }
            }
            self.commands.push(entry);
            self.replay.as_mut().unwrap().index += 1;
        }
    }
    /// Execute every fixed step. Explicit skips move the workout cursor, not the active replay clock.
    pub fn advance(&mut self, seconds: f64, rate: f64) {
        if !seconds.is_finite() || !rate.is_finite() || !(1.0..=60.0).contains(&rate) {
            return;
        }
        if self.replay_hold {
            return;
        }
        self.pump_replay();
        if self.session.paused() || self.session.phase() == Phase::Finished {
            return;
        }
        self.accumulator += seconds.clamp(0.0, 0.25) * rate;
        while self.accumulator + 1e-12 >= STEP.as_secs_f64() {
            self.pump_replay();
            if self
                .replay
                .as_ref()
                .is_some_and(|r| self.session.active_time() >= r.end)
            {
                self.message = "Replay finished.".into();
                self.accumulator = 0.0;
                break;
            }
            if self.session.paused() || self.session.phase() == Phase::Finished {
                self.accumulator = 0.0;
                break;
            }
            let input = if let Some(r) = &self.replay {
                r.input.clone().expect("validated replay input")
            } else {
                (
                    self.manual.unwrap_or_else(|| self.session.target_w()),
                    self.slot,
                    self.wheel.clone(),
                )
            };
            if self.replay.is_none() && self.last_input.as_ref() != Some(&input) {
                self.record(Action::Input {
                    watts: input.0,
                    slot: input.1,
                    wheel: input.2.clone(),
                });
                self.last_input = Some(input.clone());
            }
            match self.session.step(input.0, input.1, input.2.as_deref()) {
                Ok(sample) => {
                    self.steps += 1;
                    self.in_band += u32::from(sample.in_band);
                    if self.session.active_time().as_millis().is_multiple_of(1000) {
                        self.samples.push(sample);
                    }
                    self.accumulator = (self.accumulator - STEP.as_secs_f64()).max(0.0);
                }
                Err(e) => {
                    self.message = format!("Session stopped: {e}");
                    self.accumulator = 0.0;
                    break;
                }
            }
        }
        self.pump_replay();
    }
    /// Schema 2 records lifecycle actions as well as software controls. Legacy policy traces reject.
    pub fn trace_json(&self) -> String {
        serde_json::to_string(&json!({"schema_version":2,"policy":"mixed-game-v2","software_only":true,
            "initial_kernel_snapshot":encode_snapshot(self.initial.template().config(),self.initial.template().state()).unwrap(),
            "end_us":self.session.active_time().as_micros(),"commands":self.commands,"samples":self.samples,
            "events":self.session.events(),"outcome":self.session.outcome(),"completion":self.session.completion()})).unwrap()
    }
    /// User-requested export; never writes activity/history data.
    pub fn export_trace(&mut self, path: &str) -> bool {
        match std::fs::write(path, self.trace_json()) {
            Ok(()) => {
                self.message = format!("Replay saved to {path}");
                true
            }
            Err(e) => {
                self.message = format!("Could not save replay: {e}");
                false
            }
        }
    }
    /// Load a bounded same-policy software replay atomically.
    pub fn load_trace(&mut self, path: &str) -> bool {
        let result = (|| {
            if std::fs::metadata(path)?.len() > 64 * 1024 * 1024 {
                return Err(std::io::Error::other("Replay exceeds 64 MiB"));
            }
            std::fs::read_to_string(path)
        })();
        match result {
            Ok(text) => self.load_json(&text),
            Err(e) => {
                self.message = format!("Could not load replay: {e}");
                false
            }
        }
    }
    fn load_json(&mut self, text: &str) -> bool {
        let result = (|| -> Result<Replay, String> {
            let f: ReplayFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
            if f.schema_version != 2
                || f.policy != "mixed-game-v2"
                || !f.software_only
                || f.initial_kernel_snapshot
                    != encode_snapshot(
                        self.initial.template().config(),
                        self.initial.template().state(),
                    )
                    .unwrap()
            {
                return Err("Replay does not match this game/policy version.".into());
            }
            if f.end_us > 3_600_000_000
                || !f.end_us.is_multiple_of(50_000)
                || f.commands.len() > 100_000
            {
                return Err("Invalid replay duration/count.".into());
            }
            let (mut time, mut paused, mut input, mut skipped, mut finished) =
                (0, false, false, None, false);
            for e in &f.commands {
                if e.at_us < time
                    || e.at_us > f.end_us
                    || !e.at_us.is_multiple_of(50_000)
                    || finished
                    || (e.at_us > time && (paused || !input))
                {
                    return Err("Invalid replay time/order.".into());
                }
                let workout = skipped.map_or(e.at_us, |at: u64| 3_300_000_000 + e.at_us - at);
                if workout >= 3_600_000_000 {
                    return Err("Replay action follows natural completion.".into());
                }
                match &e.action {
                    Action::Input { watts, slot, wheel } => {
                        if e.at_us == f.end_us
                            || !watts.is_finite()
                            || !(0.0..=1800.0).contains(watts)
                            || *slot >= 3
                            || wheel
                                .as_deref()
                                .is_some_and(|w| w == PLAYER || !RIDERS.contains(&w))
                        {
                            return Err("Invalid replay input.".into());
                        }
                        input = true;
                    }
                    Action::Pause { value } => {
                        if paused == *value {
                            return Err("Redundant replay pause.".into());
                        }
                        paused = *value;
                    }
                    Action::SkipToCooldown => {
                        if workout >= 3_300_000_000 || skipped.is_some() {
                            return Err("Invalid replay skip.".into());
                        }
                        skipped = Some(e.at_us);
                    }
                    Action::Finish => {
                        finished = true;
                        paused = false;
                    }
                }
                time = e.at_us;
            }
            if f.end_us > time && (paused || finished || !input)
                || skipped.is_some_and(|at| f.end_us > at + 300_000_000)
            {
                return Err("Replay advances after stopping or completion.".into());
            }
            Ok(Replay {
                end: Duration::from_micros(f.end_us),
                commands: f.commands,
                index: 0,
                input: None,
            })
        })();
        match result {
            Ok(replay) => {
                self.reset();
                self.replay = Some(replay);
                self.message = "Playing recorded game controls.".into();
                true
            }
            Err(e) => {
                self.message = format!("Could not load replay: {e}");
                false
            }
        }
    }
    /// Race-only route elevation preview.
    pub fn elevations(&self) -> Vec<f64> {
        elevation_table(self.initial.template().config().terrain(), 45000.0)
    }
    /// Prepared presentation, with provisional game results separate from workout completion.
    pub fn view(&self) -> MixedView {
        let knowledge = session_director::mixed::knowledge();
        let phase = self.session.phase();
        let at_bus = phase != Phase::Race;
        let riders = if !at_bus {
            self.session
                .race_state()
                .unwrap()
                .riders()
                .iter()
                .map(|r| RaceRider {
                    id: r.id().as_str().into(),
                    distance_m: r.distance_m(),
                    speed_ms: r.speed_ms(),
                    slot: r.lateral_slot(),
                    draft_target: r.draft_target_id().map(|w| w.as_str().into()),
                })
                .collect()
        } else {
            vec![RaceRider {
                id: PLAYER.into(),
                distance_m: 120.0,
                speed_ms: 0.0,
                slot: 1,
                draft_target: None,
            }]
        };
        let last = self.session.last_sample();
        let race_text = last
            .and_then(|s| s.race.as_ref())
            .map(|r| {
                format!(
                    "{:?} · {:?}\n{:?}\n{}",
                    r.observation.topology(),
                    r.observation.pursuit(),
                    r.scene,
                    r.riders
                        .iter()
                        .filter(|r| r.id != PLAYER)
                        .map(|r| format!(
                            "{} · {:.0} W · {:.0}% reserve",
                            knowledge.rider(&r.id).unwrap().name(),
                            r.requested_w,
                            r.reserve_j / 200.0
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
            .unwrap_or_default();
        let phase_name = match phase {
            Phase::WarmUp => "WARM-UP · TEAM BUS",
            Phase::Race => "RACE · TEAM OBJECTIVE",
            Phase::CoolDown => "COOLDOWN · TEAM BUS",
            Phase::Finished => "SESSION ENDED",
        };
        let result=self.session.outcome().map(|o| {
            let representatives=o.represented().iter().map(|id|knowledge.rider(id).unwrap().name()).collect::<Vec<_>>().join(", ");
            format!("Game result: {:?} · {:?}\nEmber at the front: {}\nWorkout: {:?}",o.verdict(),o.reason(),if representatives.is_empty(){"none"}else{&representatives},self.session.completion())
        }).unwrap_or_else(||"Objective: at least one Ember rider, including you, in the leading group at the 45-minute competitive deadline.".into());
        MixedView {context:format!("Game prototype · provisional world model\n{}\n{}",knowledge.briefing(),knowledge.riders().iter().map(|r|format!("{} · {} · {:?}",r.name(),r.team().unwrap_or("Unaffiliated"),r.strength())).collect::<Vec<_>>().join("\n")),
            clock:format!("Workout {} / 60:00 · Active {}",format_duration(self.session.workout_time()),format_duration(self.session.active_time())),phase:phase_name.into(),
            instruction:match phase {Phase::WarmUp=>format!("Prepare on the trainer · Reference {:.0} W. The competitive field has not started.",self.session.target_w()),Phase::Race=>"Keep Ember represented at the front. Decisions and actual software power determine the outcome.".into(),Phase::CoolDown=>format!("Recover on the trainer · Reference {:.0} W. The competitive result is frozen.",self.session.target_w()),Phase::Finished=>"Review the result or restart.".into()},
            result,diagnostics:format!("{} · {}\nTarget {:.0} W · Delivered {:.0} W\nPlayer reserve {:.0}% · Power in band {:.0}%\n{}",self.session.block()+1,session_director::mixed::BLOCK_NAMES[self.session.block()],self.session.target_w(),last.map_or(0.0,|s|s.delivered_w),self.session.reserve_j()/200.0,f64::from(self.in_band)/f64::from(self.steps.max(1))*100.0,race_text),
            message:if self.session.paused()||self.replay_hold{"PAUSED".into()}else{self.message.clone()},complete:phase==Phase::Finished,paused:self.session.paused()||self.replay_hold,at_bus,riders}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_crosses_race_start_and_freezes_the_same_interrupted_field() {
        let mut a = MixedRace::new();
        for _ in 0..40 {
            a.advance(0.25, 60.0);
        }
        a.set_power(Some(330.0));
        a.set_slot(2);
        a.set_wheel("pacer");
        a.advance(0.25, 60.0);
        a.skip_to_cooldown();
        a.set_power(Some(0.0));
        a.advance(0.25, 60.0);
        a.finish();
        let mut b = MixedRace::new();
        assert!(b.load_json(&a.trace_json()));
        assert!(b.set_paused(true));
        b.advance(0.25, 60.0);
        assert_eq!(b.session.active_time(), Duration::ZERO);
        assert!(b.set_paused(false));
        for _ in 0..42 {
            b.advance(0.25, 60.0);
        }
        assert_eq!(a.session.race_state(), b.session.race_state());
        assert_eq!(a.session.outcome(), b.session.outcome());
        assert_eq!(a.commands, b.commands);
        assert_eq!(a.samples, b.samples);
    }
    #[test]
    fn replay_preserves_pause_skip_finish_and_partial_outcome() {
        let mut a = MixedRace::new();
        a.advance(0.25, 10.0);
        a.set_power(Some(600.0));
        a.advance(0.25, 10.0);
        a.set_paused(true);
        let before = a.session.active_time();
        a.advance(0.25, 60.0);
        assert_eq!(a.session.active_time(), before);
        a.set_power(Some(0.0));
        a.set_paused(false);
        a.skip_to_cooldown();
        a.advance(0.25, 10.0);
        a.finish();
        let mut b = MixedRace::new();
        assert!(b.load_json(&a.trace_json()));
        assert!(!b.set_power(Some(900.0)));
        for _ in 0..30 {
            b.advance(0.25, 1.0);
        }
        assert_eq!(a.samples, b.samples);
        assert_eq!(a.commands, b.commands);
        assert_eq!(a.session.outcome(), b.session.outcome());
        assert_eq!(a.session.completion(), b.session.completion());
        assert_eq!(a.session.reserve_j(), b.session.reserve_j());
    }
    #[test]
    fn invalid_replay_is_atomic_and_empty_replay_is_valid() {
        let mut a = MixedRace::new();
        a.advance(0.25, 10.0);
        let before = a.session.active_time();
        let base: serde_json::Value = serde_json::from_str(&a.trace_json()).unwrap();
        for (key, value) in [
            ("schema_version", json!(1)),
            ("policy", json!("mixed-v1")),
            ("end_us", json!(1)),
            ("commands", json!([])),
        ] {
            let mut bad = base.clone();
            bad[key] = value;
            assert!(!a.load_json(&bad.to_string()));
            assert_eq!(a.session.active_time(), before);
        }
        let mut bad = base.clone();
        bad["commands"][0]["action"]["slot"] = json!(3);
        assert!(!a.load_json(&bad.to_string()));
        assert!(!a.set_power(Some(f64::NAN)));
        assert!(!a.set_wheel("unknown"));
        assert!(!a.set_slot(3));
        a.reset();
        let empty = a.trace_json();
        assert!(a.load_json(&empty));
        a.advance(0.25, 60.0);
        assert_eq!(a.session.active_time(), Duration::ZERO);
    }
    #[test]
    fn frame_slicing_preserves_fixed_steps_and_bus_has_no_field() {
        let mut a = MixedRace::new();
        let mut b = MixedRace::new();
        let mut c = MixedRace::new();
        for _ in 0..40 {
            a.advance(0.25, 1.0);
        }
        for _ in 0..1000 {
            b.advance(0.01, 1.0);
        }
        for _ in 0..4 {
            c.advance(0.25, 10.0);
        }
        assert_eq!(a.samples, b.samples);
        assert_eq!(a.samples, c.samples);
        assert!(a.session.race_state().is_none());
        assert_eq!(a.view().riders.len(), 1);
    }
}
