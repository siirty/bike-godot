use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use trainer_control::{
    ManualPowerSource, SimulationParameters, TrainerConnectionState, TrainerControl, TrainerError,
    TrainerEvent,
};
use trainer_core::simulation::{RiderParams, SimDebugControl, SimulatedTrainerConnection};
use trainer_core::TrainerConnection as _;

use crate::convert;

/// The simulator's physics tick.
pub const SIM_TICK: Duration = Duration::from_millis(100);
/// How often it reports, like a typical trainer's 2 Hz notifications.
pub const SIM_REPORT_INTERVAL: Duration = Duration::from_millis(500);

/// The core simulator's internal state, for the SIM debug panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimDebugSnapshot {
    /// The power it was last given, W.
    pub rider_power_w: f64,
    /// Speed after the last tick, m/s.
    pub speed_mps: f64,
    /// Distance, m.
    pub total_distance_m: f64,
    /// Ticks since start.
    pub elapsed_ticks: u64,
    /// Whether it is running.
    pub running: bool,
    /// The SIM parameters it last received.
    pub params: SimulationParameters,
}

/// The `trainer_core` simulator as a [`TrainerControl`]: the "ride without a trainer" path.
///
/// It goes through the same SIM-parameter surface a real trainer gets, with momentum physics
/// (speed changes over time instead of jumping to the steady state). Unlike a real trainer, it
/// has no clock of its own: [`TrainerControl::poll`] advances it by whole report intervals up to
/// `now`, so a stalled frame catches up instead of drifting, and tests need no sleeps.
pub struct CoreSimulatedTrainer {
    sim: SimulatedTrainerConnection,
    state: TrainerConnectionState,
    /// When the next report falls due; `None` while not running.
    next_report_at: Option<Duration>,
    pending: Vec<TrainerEvent>,
}

impl std::fmt::Debug for CoreSimulatedTrainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreSimulatedTrainer")
            .field("state", &self.state)
            .field("debug", &self.debug_snapshot())
            .finish()
    }
}

/// Every `trainer_core` simulator method is synchronous behind its `async` signature; this polls
/// one once and insists it finished.
fn ready<T>(future: impl Future<Output = T>) -> T {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => unreachable!("the core simulator never awaits"),
    }
}

impl CoreSimulatedTrainer {
    /// A stopped, disconnected simulator for a rider of this mass, Crr and CdA.
    pub fn new(total_mass_kg: f64, crr: f64, cda_m2: f64) -> Self {
        let report_every = (SIM_REPORT_INTERVAL.as_micros() / SIM_TICK.as_micros()) as u32;
        Self {
            sim: SimulatedTrainerConnection::new(
                RiderParams {
                    total_mass_kg,
                    crr,
                    cda_m2,
                },
                SIM_TICK,
                report_every,
            ),
            state: TrainerConnectionState::Disconnected,
            next_report_at: None,
            pending: Vec::new(),
        }
    }

    fn set_state(&mut self, state: TrainerConnectionState) {
        if self.state != state {
            self.state = state;
            self.pending.push(TrainerEvent::ConnectionState(state));
        }
    }

    fn require_connected(&self) -> Result<(), TrainerError> {
        if self.state == TrainerConnectionState::Connected {
            Ok(())
        } else {
            Err(TrainerError::new("the simulated trainer is not connected"))
        }
    }

    /// The internal state.
    pub fn debug_snapshot(&self) -> SimDebugSnapshot {
        let s = self.sim.sim_debug().snapshot();
        SimDebugSnapshot {
            rider_power_w: s.rider_power_w,
            speed_mps: s.speed_mps,
            total_distance_m: s.total_distance_m,
            elapsed_ticks: s.elapsed_ticks,
            running: s.running,
            params: SimulationParameters {
                grade_fraction: s.params.grade.as_fraction(),
                headwind_ms: s.params.wind_speed_mps,
                crr: s.params.crr,
                cda_m2: s.params.cda.as_m2(),
            },
        }
    }
}

impl ManualPowerSource for CoreSimulatedTrainer {
    fn set_rider_power_watts(&mut self, watts: f64) {
        // The core takes what it is given; a dev slider must not feed it nonsense.
        let watts = if watts.is_finite() {
            watts.max(0.0)
        } else {
            0.0
        };
        self.sim.set_rider_power_w(watts);
    }
}

impl TrainerControl for CoreSimulatedTrainer {
    fn connection_state(&self) -> TrainerConnectionState {
        self.state
    }

    fn connect(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.set_state(TrainerConnectionState::Connected);
        Ok(())
    }

    fn set_simulation_parameters(
        &mut self,
        params: SimulationParameters,
        _now: Duration,
    ) -> Result<(), TrainerError> {
        self.require_connected()?;
        ready(self.sim.set_sim_params(&convert::sim_params(&params)))
            .map_err(|e| TrainerError::new(e.to_string()))
    }

    fn start(&mut self, now: Duration) -> Result<(), TrainerError> {
        self.require_connected()?;
        ready(self.sim.start_or_resume()).map_err(|e| TrainerError::new(e.to_string()))?;
        self.next_report_at.get_or_insert(now + SIM_REPORT_INTERVAL);
        Ok(())
    }

    fn stop(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.require_connected()?;
        ready(self.sim.stop_or_pause(true)).map_err(|e| TrainerError::new(e.to_string()))?;
        self.next_report_at = None;
        Ok(())
    }

    fn poll(&mut self, now: Duration) -> Vec<TrainerEvent> {
        while let Some(due) = self.next_report_at.filter(|due| *due <= now) {
            self.next_report_at = Some(due + SIM_REPORT_INTERVAL);
            match ready(self.sim.next_telemetry()) {
                Ok(Some(t)) => self
                    .pending
                    .push(TrainerEvent::Telemetry(convert::telemetry(&t))),
                Ok(None) => {}
                Err(e) => {
                    log::warn!("simulated trainer tick failed: {e}");
                    self.next_report_at = None;
                }
            }
        }
        std::mem::take(&mut self.pending)
    }

    fn dispose(&mut self) {
        let _ = ready(self.sim.disconnect());
        self.next_report_at = None;
        self.set_state(TrainerConnectionState::Disconnected);
    }

    fn manual_power_source(&mut self) -> Option<&mut dyn ManualPowerSource> {
        Some(self)
    }
}
