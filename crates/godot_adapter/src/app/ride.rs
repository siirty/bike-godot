//! A live ride: the trainer, the session, the route, and the HUD text.

use std::time::Duration;

use ble_trainer::{BleTrainer, CoreSimulatedTrainer, SimDebugSnapshot};
use race_engine::{RideFrame, RideSession, DEFAULT_RENDER_INTERVAL};
use race_physics::RiderPhysicalParams;
use session_director::{RiderProfile, TerrainSource};
use trainer_control::{
    ManualPowerSource, SimulationParameters, TrainerConnectionState, TrainerControl, TrainerError,
    TrainerEvent,
};
use workout_import::WorkoutPlan;

use crate::app::format::{
    describe_segment, format_cadence, format_duration, format_power, format_speed,
};
use crate::app::route::{grade_label, Route};

/// The trainer a ride runs against: the core simulator or a real one. One type, so the ride loop
/// never asks which.
#[derive(Debug)]
pub enum RideTrainer {
    /// "Ride without a trainer".
    Simulated(CoreSimulatedTrainer),
    /// Bluetooth FTMS, SIM mode.
    Ble(BleTrainer),
}

impl RideTrainer {
    fn inner(&mut self) -> &mut dyn TrainerControl {
        match self {
            RideTrainer::Simulated(t) => t,
            RideTrainer::Ble(t) => t,
        }
    }

    /// Failures since the last call, for the rider.
    pub fn take_failures(&mut self) -> Vec<String> {
        match self {
            RideTrainer::Simulated(_) => Vec::new(),
            RideTrainer::Ble(t) => t.take_failures().into_iter().map(|f| f.message).collect(),
        }
    }

    /// The simulator's internals, when this is the simulator.
    pub fn sim_debug(&self) -> Option<SimDebugSnapshot> {
        match self {
            RideTrainer::Simulated(t) => Some(t.debug_snapshot()),
            RideTrainer::Ble(_) => None,
        }
    }
}

impl TrainerControl for RideTrainer {
    fn connection_state(&self) -> TrainerConnectionState {
        match self {
            RideTrainer::Simulated(t) => t.connection_state(),
            RideTrainer::Ble(t) => t.connection_state(),
        }
    }
    fn connect(&mut self, now: Duration) -> Result<(), TrainerError> {
        self.inner().connect(now)
    }
    fn set_simulation_parameters(
        &mut self,
        params: SimulationParameters,
        now: Duration,
    ) -> Result<(), TrainerError> {
        self.inner().set_simulation_parameters(params, now)
    }
    fn start(&mut self, now: Duration) -> Result<(), TrainerError> {
        self.inner().start(now)
    }
    fn stop(&mut self, now: Duration) -> Result<(), TrainerError> {
        self.inner().stop(now)
    }
    fn poll(&mut self, now: Duration) -> Vec<TrainerEvent> {
        self.inner().poll(now)
    }
    fn dispose(&mut self) {
        self.inner().dispose();
    }
    fn manual_power_source(&mut self) -> Option<&mut dyn ManualPowerSource> {
        self.inner().manual_power_source()
    }
}

/// `connecting`, `connected`, …: what GDScript branches on.
pub fn state_name(state: TrainerConnectionState) -> &'static str {
    match state {
        TrainerConnectionState::Disconnected => "disconnected",
        TrainerConnectionState::Connecting => "connecting",
        TrainerConnectionState::Connected => "connected",
        TrainerConnectionState::ConnectionLost => "connection_lost",
    }
}

/// The connect step: a trainer being connected, before a ride takes it over.
#[derive(Debug, Default)]
pub struct TrainerLink {
    trainer: Option<RideTrainer>,
    messages: Vec<String>,
}

impl TrainerLink {
    /// The core simulator for this rider.
    pub fn simulated(rider: &RiderPhysicalParams) -> Self {
        Self::with(RideTrainer::Simulated(CoreSimulatedTrainer::new(
            rider.total_mass_kg,
            rider.crr,
            rider.cda_m2,
        )))
    }

    /// A Bluetooth trainer.
    pub fn ble(trainer: BleTrainer) -> Self {
        Self::with(RideTrainer::Ble(trainer))
    }

    fn with(trainer: RideTrainer) -> Self {
        Self {
            trainer: Some(trainer),
            messages: Vec::new(),
        }
    }

    /// Starts connecting.
    pub fn connect(&mut self, now: Duration) -> Result<(), String> {
        let trainer = self.trainer.as_mut().ok_or("no trainer to connect")?;
        trainer.connect(now).map_err(|e| e.message)
    }

    /// Advances the connection and reports where it is. Telemetry before the ride is discarded.
    pub fn update(&mut self, now: Duration) -> TrainerConnectionState {
        let Some(trainer) = self.trainer.as_mut() else {
            return TrainerConnectionState::Disconnected;
        };
        trainer.poll(now);
        self.messages.extend(trainer.take_failures());
        trainer.connection_state()
    }

    /// Failure messages since the last call.
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }

    /// Hands the trainer to a ride.
    pub fn take_trainer(&mut self) -> Option<RideTrainer> {
        self.trainer.take()
    }
}

impl Drop for TrainerLink {
    fn drop(&mut self) {
        if let Some(t) = self.trainer.as_mut() {
            t.dispose();
        }
    }
}

/// One HUD frame, already worded.
#[derive(Debug, Clone, PartialEq)]
pub struct HudFrame {
    /// The session's frame.
    pub frame: RideFrame,
    /// Grade the trainer was last asked for at this distance.
    pub grade: f64,
    /// Texts.
    pub power_text: String,
    /// .
    pub speed_text: String,
    /// .
    pub cadence_text: String,
    /// .
    pub elapsed_text: String,
    /// .
    pub segment_text: String,
    /// `Target: 250 W` or `Target: free ride`.
    pub target_text: String,
    /// `Remaining in segment: 2:10`.
    pub remaining_text: String,
    /// `+3.2%`.
    pub grade_text: String,
}

/// A ride in progress. Owns its trainer: ending the ride releases it.
pub struct Ride {
    session: Option<RideSession<RideTrainer>>,
    route: Route,
    latest: Option<HudFrame>,
    messages: Vec<String>,
}

impl std::fmt::Debug for Ride {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ride")
            .field("latest", &self.latest)
            .finish_non_exhaustive()
    }
}

impl Ride {
    /// Starts riding `plan` on `trainer`.
    pub fn start(
        plan: WorkoutPlan,
        rider: RiderProfile,
        trainer: RideTrainer,
        now: Duration,
    ) -> Result<Self, String> {
        let route = Route::synthesize(&plan, &rider)?;
        let terrain: Box<dyn TerrainSource> = Box::new(route.terrain.clone());
        let mut session = RideSession::new(plan, rider, trainer, terrain, DEFAULT_RENDER_INTERVAL)
            .map_err(|e| e.to_string())?;
        session
            .start(now)
            .map_err(|e| format!("Could not start the trainer: {e}"))?;
        Ok(Self {
            session: Some(session),
            route,
            latest: None,
            messages: Vec::new(),
        })
    }

    fn session(&mut self) -> &mut RideSession<RideTrainer> {
        self.session
            .as_mut()
            .expect("a ride has its session until it is dropped")
    }

    /// Advances to `now` and returns the newest frame.
    pub fn advance(&mut self, now: Duration) -> Option<&HudFrame> {
        let advance = self.session().advance(now);
        for state in advance.connection_states {
            match state {
                TrainerConnectionState::ConnectionLost => {
                    self.messages.push("Trainer connection lost.".into())
                }
                TrainerConnectionState::Connected => {
                    self.messages.push("Trainer connected.".into())
                }
                _ => {}
            }
        }
        self.messages
            .extend(advance.write_errors.into_iter().map(|e| e.message));
        let failures = self.session().trainer_mut().inner_mut().take_failures();
        self.messages.extend(failures);
        if let Some(frame) = advance.frames.into_iter().last() {
            let grade = self.route.terrain.grade_at_distance(frame.distance_m);
            self.latest = Some(HudFrame {
                power_text: format_power(frame.power_watts),
                speed_text: format_speed(frame.speed_ms),
                cadence_text: format_cadence(frame.cadence_rpm),
                elapsed_text: format!(
                    "{} / {}",
                    format_duration(frame.elapsed),
                    format_duration(self.route_duration())
                ),
                segment_text: describe_segment(&frame.current_segment),
                target_text: frame.target_power_watts.map_or_else(
                    || "Target: free ride".to_owned(),
                    |w| format!("Target: {}", format_power(w)),
                ),
                remaining_text: format!(
                    "Remaining in segment: {}",
                    format_duration(frame.time_remaining_in_segment)
                ),
                grade_text: grade_label(grade),
                grade,
                frame,
            });
        }
        self.latest.as_ref()
    }

    fn route_duration(&mut self) -> Duration {
        self.session().total_duration()
    }

    /// Whether the plan is finished.
    pub fn is_complete(&self) -> bool {
        self.session.as_ref().is_some_and(RideSession::is_complete)
    }

    /// The route being ridden.
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// Whether a power slider makes sense (the simulator only).
    pub fn has_manual_power(&mut self) -> bool {
        self.session().trainer_mut().manual_power_source().is_some()
    }

    /// Sets the simulator's rider power. Ignored on a real trainer, which measures the rider.
    pub fn set_manual_power(&mut self, watts: f64) {
        if let Some(source) = self.session().trainer_mut().manual_power_source() {
            source.set_rider_power_watts(watts);
        }
    }

    /// The simulator's internals.
    pub fn sim_debug(&mut self) -> Option<SimDebugSnapshot> {
        self.session().trainer().inner().sim_debug()
    }

    /// Messages for the rider since the last call.
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }
}

impl Drop for Ride {
    fn drop(&mut self) {
        // The ride holds the trainer's lease: leaving the ride, by any route, releases it.
        if let Some(session) = self.session.take() {
            session.into_trainer().dispose();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use workout_import::{PowerTarget, WorkoutSegment};

    fn plan() -> WorkoutPlan {
        WorkoutPlan {
            segments: vec![
                WorkoutSegment::SteadyState {
                    duration: Duration::from_secs(20),
                    target_power: PowerTarget::PercentFtp(0.6),
                    cadence: None,
                },
                WorkoutSegment::SteadyState {
                    duration: Duration::from_secs(20),
                    target_power: PowerTarget::PercentFtp(1.0),
                    cadence: None,
                },
            ],
            ..WorkoutPlan::default()
        }
    }

    #[test]
    fn a_simulated_ride_runs_to_completion_through_the_real_loop() {
        let rider = RiderProfile::new(200.0, RiderPhysicalParams::new(80.0));
        let mut link = TrainerLink::simulated(&rider.physical);
        link.connect(Duration::ZERO).unwrap();
        assert_eq!(
            link.update(Duration::ZERO),
            TrainerConnectionState::Connected
        );
        let mut ride =
            Ride::start(plan(), rider, link.take_trainer().unwrap(), Duration::ZERO).unwrap();
        assert!(ride.has_manual_power());
        ride.set_manual_power(200.0);

        let mut t = Duration::ZERO;
        let mut saw_second_segment = false;
        while !ride.is_complete() {
            t += Duration::from_millis(16);
            assert!(t < Duration::from_secs(45), "finishes on time");
            if let Some(hud) = ride.advance(t) {
                saw_second_segment |= hud.frame.segment_index == 1;
            }
        }
        let hud = ride.advance(t).unwrap().clone();
        assert!(hud.frame.ride_complete && saw_second_segment);
        assert!(hud.frame.distance_m > 100.0, "{}", hud.frame.distance_m);
        assert_eq!(hud.target_text, "Target: 200 W");
        let debug = ride.sim_debug().unwrap();
        assert_eq!(debug.rider_power_w, 200.0);
        assert!(
            debug.params.grade_fraction > 0.0,
            "the threshold block climbs"
        );
    }
}
