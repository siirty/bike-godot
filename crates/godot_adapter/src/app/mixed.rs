//! Software-only input, fixed-step playback and versioned audit. No trainer/history access.
use crate::app::{format::format_duration, race::RaceRider, route::elevation_table};
use race_engine::{encode_snapshot, mixed::*, RaceState};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{sync::OnceLock, time::Duration};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    at_us: u64,
    watts: f64,
    slot: usize,
    wheel: Option<String>,
}
#[derive(Deserialize)]
struct ReplayFile {
    schema_version: u32,
    policy: String,
    software_only: bool,
    initial_kernel_snapshot: String,
    end_us: u64,
    commands: Vec<Control>,
}
struct Replay {
    end: Duration,
    commands: Vec<Control>,
    index: usize,
}
/// Plain Rust presentation; GDScript arranges these values without domain calculations.
pub struct MixedView {
    /// Essential objective and qualitative roster, always available.
    pub context: String,
    /// Public time and speed.
    pub clock: String,
    /// Hidden until coaching/private diagnostics are explicitly revealed.
    pub diagnostics: String,
    /// Simulation/control/export status.
    pub message: String,
    /// True only after the actual hour has completed.
    pub complete: bool,
    /// Interpolated actual kernel riders.
    pub riders: Vec<RaceRider>,
}
/// Application continuation for the mixed-workout experiment.
pub struct MixedRace {
    initial: MixedSession,
    session: MixedSession,
    previous: RaceState,
    accumulator: f64,
    manual: Option<f64>,
    slot: usize,
    wheel: Option<String>,
    samples: Vec<Sample>,
    commands: Vec<Control>,
    replay: Option<Replay>,
    message: String,
    steps: u32,
    in_band: u32,
    sheltered: u32,
}
impl Default for MixedRace {
    fn default() -> Self {
        Self::new()
    }
}
impl MixedRace {
    /// Cache the immutable offline plan; reopening or restarting never repeats fitting.
    pub fn new() -> Self {
        static INITIAL: OnceLock<MixedSession> = OnceLock::new();
        Self::from_initial(
            INITIAL
                .get_or_init(|| MixedSession::new(MixedOptions::default()).expect("valid fixture"))
                .clone(),
        )
    }
    fn from_initial(initial: MixedSession) -> Self {
        Self {
            session: initial.clone(),
            previous: initial.state().clone(),
            initial,
            accumulator: 0.0,
            manual: None,
            slot: 1,
            wheel: None,
            samples: vec![],
            commands: vec![],
            replay: None,
            message: String::new(),
            steps: 0,
            in_band: 0,
            sheltered: 0,
        }
    }
    /// Clear simulation/policy/audit continuation; retain the visible user control selections.
    pub fn reset(&mut self) {
        self.session = self.initial.clone();
        self.previous = self.session.state().clone();
        self.accumulator = 0.0;
        self.samples.clear();
        self.commands.clear();
        self.replay = None;
        self.message.clear();
        self.steps = 0;
        self.in_band = 0;
        self.sheltered = 0;
    }
    /// Choose manual software watts, or None for the nominal prescribed reference.
    pub fn set_power(&mut self, power: Option<f64>) -> bool {
        if self.replay.is_some() {
            self.message = "Restart to leave replay before changing controls.".into();
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
    /// Request a valid road slot; no position is written.
    pub fn set_slot(&mut self, slot: i64) -> bool {
        if self.replay.is_some() || !(0..3).contains(&slot) {
            return false;
        }
        self.slot = slot as usize;
        true
    }
    /// Request an eligible known wheel. Eligibility is resolved by the kernel.
    pub fn set_wheel(&mut self, name: &str) -> bool {
        if self.replay.is_some()
            || (!name.is_empty() && (name == PLAYER || !RIDERS.contains(&name)))
        {
            return false;
        }
        self.wheel = (!name.is_empty()).then(|| name.into());
        true
    }
    /// Accumulate explicit real time, clamping stalls and preserving every 50 ms kernel step.
    pub fn advance(&mut self, seconds: f64, rate: f64) {
        if self.session.is_complete()
            || !seconds.is_finite()
            || !rate.is_finite()
            || !(1.0..=60.0).contains(&rate)
        {
            return;
        }
        self.accumulator += seconds.clamp(0.0, 0.25) * rate;
        while self.accumulator + 1e-12 >= 0.05 && !self.session.is_complete() {
            let time = self.session.state().simulation_time();
            let control = if let Some(replay) = &mut self.replay {
                if time >= replay.end {
                    self.accumulator = 0.0;
                    self.message = "Replay finished.".into();
                    break;
                }
                while replay.index + 1 < replay.commands.len()
                    && replay.commands[replay.index + 1].at_us <= time.as_micros() as u64
                {
                    replay.index += 1;
                }
                replay.commands[replay.index].clone()
            } else {
                Control {
                    at_us: time.as_micros() as u64,
                    watts: self.manual.unwrap_or_else(|| self.session.target_w()),
                    slot: self.slot,
                    wheel: self.wheel.clone(),
                }
            };
            let previous = self.session.state().clone();
            match self
                .session
                .step(control.watts, control.slot, control.wheel.as_deref())
            {
                Ok(step) => {
                    if self.commands.last().is_none_or(|c| {
                        c.watts != control.watts
                            || c.slot != control.slot
                            || c.wheel != control.wheel
                    }) {
                        self.commands.push(Control {
                            at_us: time.as_micros() as u64,
                            ..control
                        });
                    }
                    self.previous = previous;
                    self.accumulator = (self.accumulator - 0.05).max(0.0);
                    self.steps += 1;
                    self.in_band += u32::from(step.sample.in_band);
                    self.sheltered += u32::from(
                        step.sample
                            .riders
                            .iter()
                            .find(|r| r.id == PLAYER)
                            .unwrap()
                            .wheel
                            .is_some(),
                    );
                    if self
                        .session
                        .state()
                        .simulation_time()
                        .as_millis()
                        .is_multiple_of(1000)
                    {
                        self.samples.push(step.sample);
                    }
                }
                Err(error) => {
                    self.message = format!("Session stopped: {error}");
                    self.accumulator = 0.0;
                    break;
                }
            }
        }
    }
    /// Versioned controls plus diagnostics, explicitly simulated; no activity samples are created.
    pub fn trace_json(&self) -> String {
        serde_json::to_string(&json!({"schema_version":1,"policy":"mixed-v1","software_only":true,
            "initial_kernel_snapshot":encode_snapshot(self.initial.config(),self.initial.state()).unwrap(),
            "end_us":self.session.state().simulation_time().as_micros(),"commands":self.commands,"samples":self.samples})).unwrap()
    }
    /// Write only when explicitly requested by the user.
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
    /// Read a bounded software replay; invalid data does not reset the current race.
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
            let file: ReplayFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
            if file.schema_version != 1
                || file.policy != "mixed-v1"
                || !file.software_only
                || file.initial_kernel_snapshot
                    != encode_snapshot(self.initial.config(), self.initial.state()).unwrap()
            {
                return Err("Replay does not match this session/policy version.".into());
            }
            if file.end_us > 3_600_000_000
                || !file.end_us.is_multiple_of(50_000)
                || file.commands.len() > 72000
                || (file.end_us > 0 && file.commands.is_empty())
                || (file.end_us == 0 && !file.commands.is_empty())
            {
                return Err("Invalid replay duration/control count.".into());
            }
            let mut previous = None;
            for c in &file.commands {
                if c.at_us >= file.end_us
                    || !c.at_us.is_multiple_of(50_000)
                    || previous.is_some_and(|p| c.at_us <= p)
                    || (previous.is_none() && c.at_us != 0)
                    || !c.watts.is_finite()
                    || !(0.0..=1800.0).contains(&c.watts)
                    || c.slot >= 3
                    || c.wheel
                        .as_deref()
                        .is_some_and(|w| w == PLAYER || !RIDERS.contains(&w))
                {
                    return Err("Invalid replay controls or time order.".into());
                }
                previous = Some(c.at_us);
            }
            Ok(Replay {
                end: Duration::from_micros(file.end_us),
                commands: file.commands,
                index: 0,
            })
        })();
        match result {
            Ok(replay) => {
                self.reset();
                self.replay = Some(replay);
                self.message = "Playing recorded software input.".into();
                true
            }
            Err(e) => {
                self.message = format!("Could not load replay: {e}");
                false
            }
        }
    }
    /// Course samples for the existing road renderer.
    pub fn elevations(&self) -> Vec<f64> {
        elevation_table(self.session.config().terrain(), 45000.0)
    }
    /// All domain-derived presentation text lives in Rust.
    pub fn view(&self) -> MixedView {
        let state = self.session.state();
        let knowledge = session_director::mixed::knowledge();
        let alpha = if self.session.is_complete() {
            1.0
        } else {
            (self.accumulator / 0.05).clamp(0.0, 1.0)
        };
        let riders = state
            .riders()
            .iter()
            .zip(self.previous.riders())
            .map(|(r, p)| RaceRider {
                id: r.id().as_str().into(),
                distance_m: p.distance_m() + (r.distance_m() - p.distance_m()) * alpha,
                speed_ms: r.speed_ms(),
                slot: r.lateral_slot(),
                draft_target: r.draft_target_id().map(|w| w.as_str().into()),
            })
            .collect();
        let human = state
            .riders()
            .iter()
            .find(|r| r.id().as_str() == PLAYER)
            .unwrap();
        let instruction = match self
            .session
            .last_sample()
            .map(|s| s.scene)
            .unwrap_or(Scene::WarmUp)
        {
            Scene::WarmUp => "Settle into the session",
            Scene::HoldShelter => "Hold an available wheel; watch the race",
            Scene::Pursue => "Contribute toward the rider ahead",
            Scene::MaintainContribution => "Keep your contribution steady; assess the group",
            Scene::SoloFallback => "No usable wheel: continue independently",
            Scene::SteadyContribution => "Take a measured contribution",
            Scene::ShortResponse => "Make your short contribution; assess the gap",
            Scene::CoolDown => "Competitive exercise ended; cool down",
        };
        let last = self.session.last_sample();
        let n = f64::from(self.steps.max(1));
        let npcs = last
            .map(|s| {
                s.riders
                    .iter()
                    .filter(|r| r.id != PLAYER)
                    .map(|r| {
                        format!(
                            "{} · {:.0} W · {:.0}% reserve",
                            knowledge.rider(&r.id).unwrap().name(),
                            r.requested_w,
                            r.reserve_j / 200.0
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        MixedView {
            context:format!("{}\n{}",knowledge.briefing(),knowledge.riders().iter().map(|r|format!("{} · {} · {:?}",r.name(),r.team().unwrap_or("Unaffiliated"),r.strength())).collect::<Vec<_>>().join("\n")),
            clock:format!("{} / 60:00 · {:.1} km/h",format_duration(state.simulation_time()),human.speed_ms()*3.6),
            diagnostics:format!("{} · {}\nTarget {:.0} W · Simulated {:.0} W\n{}\n{:?} · {:?}\nPower in band {:.0}% · Time sheltered {:.0}%\nSpotlight role: {}\n{}",self.session.block()+1,session_director::mixed::BLOCK_NAMES[self.session.block()],self.session.target_w(),last.map_or(0.0,|s|s.delivered_w),instruction,self.session.observation().topology(),self.session.observation().pursuit(),f64::from(self.in_band)/n*100.0,f64::from(self.sheltered)/n*100.0,last.map_or("Conserving",|s|s.spotlight_role.as_str()),npcs),
            message:self.message.clone(),complete:self.session.is_complete(),riders,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_controls_replay_exactly_at_a_different_frame_rate() {
        let mut original = MixedRace::new();
        original.advance(0.25, 10.0);
        original.set_power(Some(330.0));
        original.set_slot(2);
        original.set_wheel("pacer");
        original.advance(0.25, 10.0);
        original.set_power(Some(0.0));
        original.advance(0.25, 10.0);
        let mut replay = MixedRace::new();
        assert!(replay.load_json(&original.trace_json()));
        assert!(!replay.set_power(Some(900.0)));
        for _ in 0..30 {
            replay.advance(0.25, 1.0);
        }
        assert_eq!(replay.session.state(), original.session.state());
        assert_eq!(replay.samples, original.samples);
        assert_eq!(replay.commands, original.commands);
        let before = replay.session.state().clone();
        replay.advance(0.25, 60.0);
        assert_eq!(replay.session.state(), &before);
    }
    #[test]
    fn invalid_replay_and_controls_are_atomic_and_reset_clears_audit() {
        let mut race = MixedRace::new();
        race.advance(0.25, 10.0);
        let before = race.session.state().clone();
        let base: serde_json::Value = serde_json::from_str(&race.trace_json()).unwrap();
        assert_eq!(base["software_only"], true);
        for (key, value) in [
            ("policy", json!("unknown")),
            ("schema_version", json!(99)),
            ("end_us", json!(1)),
            ("commands", json!([])),
        ] {
            let mut bad = base.clone();
            bad[key] = value;
            assert!(!race.load_json(&bad.to_string()));
            assert_eq!(race.session.state(), &before);
        }
        for (key, value) in [
            ("slot", json!(3)),
            ("wheel", json!("unknown")),
            ("watts", json!(-1)),
            ("at_us", json!(50_000)),
        ] {
            let mut bad = base.clone();
            bad["commands"][0][key] = value;
            assert!(!race.load_json(&bad.to_string()));
            assert_eq!(race.session.state(), &before);
        }
        assert!(!race.set_power(Some(f64::NAN)));
        assert!(!race.set_wheel("unknown"));
        assert!(!race.set_slot(3));
        race.set_power(Some(0.0));
        race.advance(0.25, 1.0);
        assert_eq!(race.session.last_sample().unwrap().delivered_w, 0.0);
        race.reset();
        assert_eq!(race.session.state(), race.initial.state());
        assert!(race.commands.is_empty());
        assert!(race.samples.is_empty());
        let empty = race.trace_json();
        assert!(race.load_json(&empty));
        race.advance(0.25, 60.0);
        assert_eq!(race.session.state(), race.initial.state());
    }
    #[test]
    fn frame_slicing_preserves_all_fixed_steps() {
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
        assert_eq!(a.session.state(), b.session.state());
        assert_eq!(a.session.state(), c.session.state());
        assert_eq!(a.samples, c.samples);
    }
}
