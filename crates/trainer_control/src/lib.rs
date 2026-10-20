//! The trainer seam every ride loop depends on, never a concrete backend.
//!
//! Always FTMS **SIM mode**: the app describes the environment (grade, wind, Crr, CdA) and the
//! trainer — real firmware, or [`SimulatedTrainer`]'s stand-in physics — reports real speed and
//! power back. There is no target-power (ERG) operation on this interface, deliberately.
//!
//! ## Time is explicit
//!
//! The Dart prototype used broadcast streams and timers. Here every time-dependent call takes a
//! caller-supplied monotonic `now`, and events are pulled with [`TrainerControl::poll`]. The engine's
//! frame loop drives it, tests drive it with literal durations (what `fake_async` did in Dart), and
//! no wall clock hides inside a trainer.

mod model;
mod recording;
mod simulated;
mod throttled;

pub use model::{
    SimDebugState, SimulationParameters, TrainerConnectionState, TrainerError, TrainerEvent,
    TrainerTelemetry,
};
pub use recording::RecordingTrainerControl;
pub use simulated::{SimulatedTrainer, SimulatedTrainerOptions};
pub use throttled::{ThrottledTrainerControl, DEFAULT_MIN_GRADE_DELTA, DEFAULT_MIN_WRITE_INTERVAL};

use std::time::Duration;

/// The interface every trainer implementation speaks.
pub trait TrainerControl {
    /// The current connection state.
    fn connection_state(&self) -> TrainerConnectionState;

    /// Starts connecting. A simulated trainer connects immediately; a BLE trainer reports
    /// [`TrainerConnectionState::Connected`] through [`Self::poll`] when the handshake completes.
    fn connect(&mut self, now: Duration) -> Result<(), TrainerError>;

    /// Writes simulation parameters. Implementations write whatever they are given, every time;
    /// the write-rate policy is [`ThrottledTrainerControl`]'s alone.
    fn set_simulation_parameters(
        &mut self,
        params: SimulationParameters,
        now: Duration,
    ) -> Result<(), TrainerError>;

    /// FTMS "Start or Resume". Resuming after [`Self::stop`] is the same call and resets nothing.
    fn start(&mut self, now: Duration) -> Result<(), TrainerError>;

    /// Stops (pauses) the session.
    fn stop(&mut self, now: Duration) -> Result<(), TrainerError>;

    /// Advances the implementation to `now` and returns what happened since the last poll:
    /// telemetry reports and connection transitions, in order. `now` must not go backwards.
    fn poll(&mut self, now: Duration) -> Vec<TrainerEvent>;

    /// Releases resources and moves to [`TrainerConnectionState::Disconnected`].
    fn dispose(&mut self);

    /// The app-supplied-power capability, if this trainer has it. A real trainer measures the
    /// rider and never accepts power, so this is `None` for it. This is the dev/test power slider's
    /// way in without naming a concrete type.
    fn manual_power_source(&mut self) -> Option<&mut dyn ManualPowerSource> {
        None
    }
}

/// A source of rider power the app supplies, rather than one measured from real pedaling.
pub trait ManualPowerSource {
    /// Sets the rider's current power output, in watts.
    fn set_rider_power_watts(&mut self, watts: f64);
}

impl<T: TrainerControl + ?Sized> TrainerControl for Box<T> {
    fn connection_state(&self) -> TrainerConnectionState {
        (**self).connection_state()
    }
    fn connect(&mut self, now: Duration) -> Result<(), TrainerError> {
        (**self).connect(now)
    }
    fn set_simulation_parameters(
        &mut self,
        params: SimulationParameters,
        now: Duration,
    ) -> Result<(), TrainerError> {
        (**self).set_simulation_parameters(params, now)
    }
    fn start(&mut self, now: Duration) -> Result<(), TrainerError> {
        (**self).start(now)
    }
    fn stop(&mut self, now: Duration) -> Result<(), TrainerError> {
        (**self).stop(now)
    }
    fn poll(&mut self, now: Duration) -> Vec<TrainerEvent> {
        (**self).poll(now)
    }
    fn dispose(&mut self) {
        (**self).dispose()
    }
    fn manual_power_source(&mut self) -> Option<&mut dyn ManualPowerSource> {
        (**self).manual_power_source()
    }
}
