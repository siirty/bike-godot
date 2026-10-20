use std::time::Duration;

use crate::{
    ManualPowerSource, SimulationParameters, TrainerConnectionState, TrainerControl, TrainerError,
    TrainerEvent, TrainerTelemetry,
};

/// A trainer that records what it was asked to do and emits exactly what it is told to. For
/// interaction tests, where a simulated trainer's physics would be noise.
///
/// The manual-power capability is opt-in ([`Self::with_manual_power`]), because a real trainer
/// never accepts power and most tests should not see the capability.
#[derive(Debug, Default)]
pub struct RecordingTrainerControl {
    /// If set, `connect` fails with this message.
    pub connect_error: Option<String>,
    /// Every SIM write that reached this trainer, with its time, in order.
    pub sim_param_writes: Vec<(SimulationParameters, Duration)>,
    /// Every manual power set, if the capability is enabled.
    pub rider_power_writes: Vec<f64>,
    /// Call counts.
    pub connect_count: u32,
    /// Call counts.
    pub start_count: u32,
    /// Call counts.
    pub stop_count: u32,
    /// Call counts.
    pub dispose_count: u32,
    manual_power: bool,
    state: Option<TrainerConnectionState>,
    pending: Vec<TrainerEvent>,
}

impl RecordingTrainerControl {
    /// A recorder without the manual-power capability.
    pub fn new() -> Self {
        Self::default()
    }

    /// A recorder that also offers [`ManualPowerSource`].
    pub fn with_manual_power() -> Self {
        Self {
            manual_power: true,
            ..Self::default()
        }
    }

    /// Queues a telemetry report for the next poll.
    pub fn emit_telemetry(&mut self, telemetry: TrainerTelemetry) {
        self.pending.push(TrainerEvent::Telemetry(telemetry));
    }

    /// Moves the connection state and queues the transition.
    pub fn emit_connection_state(&mut self, state: TrainerConnectionState) {
        self.state = Some(state);
        self.pending.push(TrainerEvent::ConnectionState(state));
    }

    /// The grades written, in order.
    pub fn grades_written(&self) -> Vec<f64> {
        self.sim_param_writes
            .iter()
            .map(|(p, _)| p.grade_fraction)
            .collect()
    }
}

impl TrainerControl for RecordingTrainerControl {
    fn connection_state(&self) -> TrainerConnectionState {
        self.state.unwrap_or(TrainerConnectionState::Disconnected)
    }

    fn connect(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.connect_count += 1;
        if let Some(message) = self.connect_error.clone() {
            self.emit_connection_state(TrainerConnectionState::Disconnected);
            return Err(TrainerError::new(message));
        }
        self.emit_connection_state(TrainerConnectionState::Connected);
        Ok(())
    }

    fn set_simulation_parameters(
        &mut self,
        params: SimulationParameters,
        now: Duration,
    ) -> Result<(), TrainerError> {
        self.sim_param_writes.push((params, now));
        Ok(())
    }

    fn start(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.start_count += 1;
        Ok(())
    }

    fn stop(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.stop_count += 1;
        Ok(())
    }

    fn poll(&mut self, _now: Duration) -> Vec<TrainerEvent> {
        std::mem::take(&mut self.pending)
    }

    fn dispose(&mut self) {
        self.dispose_count += 1;
    }

    fn manual_power_source(&mut self) -> Option<&mut dyn ManualPowerSource> {
        if self.manual_power {
            Some(self)
        } else {
            None
        }
    }
}

impl ManualPowerSource for RecordingTrainerControl {
    fn set_rider_power_watts(&mut self, watts: f64) {
        self.rider_power_writes.push(watts);
    }
}
