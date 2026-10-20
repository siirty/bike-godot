use std::time::Duration;

use race_physics::{
    solve_speed_for_power, EnvironmentParams, PhysicsError, RiderPhysicalParams, SolverOptions,
    MAX_SOLVABLE_SPEED_MS,
};

use crate::{
    ManualPowerSource, SimDebugState, SimulationParameters, TrainerConnectionState, TrainerControl,
    TrainerError, TrainerEvent, TrainerTelemetry,
};

/// Construction options for [`SimulatedTrainer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulatedTrainerOptions {
    /// SIM parameters before the first write.
    pub initial_sim_params: SimulationParameters,
    /// How often speed and distance are recomputed. Default 50 ms (20 Hz).
    pub tick_interval: Duration,
    /// How often telemetry is reported. Default 500 ms (2 Hz), inside the real FTMS ~1–4 Hz range,
    /// so render smoothing has something real to interpolate between.
    pub report_interval: Duration,
    /// Whether reports carry FTMS's optional Elapsed Time. `false` models the many trainers that
    /// never set that flag, so the ride loop's own-clock path gets exercised.
    pub report_elapsed_time: bool,
}

impl Default for SimulatedTrainerOptions {
    fn default() -> Self {
        Self {
            initial_sim_params: SimulationParameters::default(),
            tick_interval: Duration::from_millis(50),
            report_interval: Duration::from_millis(500),
            report_elapsed_time: true,
        }
    }
}

/// A trainer backed by `race_physics`'s power→speed solver instead of firmware: what tests, CI and
/// hardware-free development ride against.
///
/// Each tick holds the steady-state speed for the current power (`solve_speed_for_power`). There is
/// no physical rider, so power comes in through [`ManualPowerSource`]. SIM writes apply
/// immediately: the write-rate policy protects real firmware, which this has none of.
#[derive(Debug, Clone)]
pub struct SimulatedTrainer {
    rider: RiderPhysicalParams,
    options: SimulatedTrainerOptions,
    params: SimulationParameters,
    current_env: EnvironmentParams,
    current_rider: RiderPhysicalParams,
    rider_power_watts: f64,
    total_distance_m: f64,
    elapsed_session_time: Duration,
    last_speed_ms: f64,
    time_since_last_report: Duration,
    elapsed_ticks: u64,
    next_tick_at: Option<Duration>,
    connection_state: TrainerConnectionState,
    pending: Vec<TrainerEvent>,
}

impl SimulatedTrainer {
    /// A simulated trainer for `rider`.
    ///
    /// # Panics
    /// If the tick or report interval is zero.
    pub fn new(rider: RiderPhysicalParams, options: SimulatedTrainerOptions) -> Self {
        assert!(
            !options.tick_interval.is_zero(),
            "tick interval must be positive"
        );
        assert!(
            !options.report_interval.is_zero(),
            "report interval must be positive"
        );
        let params = options.initial_sim_params;
        Self {
            rider,
            options,
            params,
            current_env: params.to_environment_params(),
            current_rider: params.apply_to(&rider),
            rider_power_watts: 0.0,
            total_distance_m: 0.0,
            elapsed_session_time: Duration::ZERO,
            last_speed_ms: 0.0,
            time_since_last_report: Duration::ZERO,
            elapsed_ticks: 0,
            next_tick_at: None,
            connection_state: TrainerConnectionState::Disconnected,
            pending: Vec::new(),
        }
    }

    /// Drops the simulated connection, as a trainer going out of range would.
    pub fn simulate_connection_loss(&mut self) {
        self.set_connection_state(TrainerConnectionState::ConnectionLost);
    }

    /// The internal state, for the SIM debug screen.
    pub fn debug_state(&self) -> SimDebugState {
        let tick = self.options.tick_interval.as_micros();
        SimDebugState {
            rider_power_w: self.rider_power_watts,
            speed_mps: self.last_speed_ms,
            total_distance_m: self.total_distance_m,
            elapsed_ticks: self.elapsed_ticks,
            tick_interval_ms: self.options.tick_interval.as_millis() as u64,
            report_every_ticks: self.options.report_interval.as_micros().div_ceil(tick) as u64,
            running: self.next_tick_at.is_some(),
            params: self.params,
        }
    }

    fn set_connection_state(&mut self, state: TrainerConnectionState) {
        if self.connection_state == state {
            return;
        }
        self.connection_state = state;
        self.pending.push(TrainerEvent::ConnectionState(state));
    }

    fn tick(&mut self) {
        self.last_speed_ms = match solve_speed_for_power(
            self.rider_power_watts,
            &self.current_rider,
            &self.current_env,
            SolverOptions::default(),
        ) {
            Ok(speed) => speed,
            // More power than 108 km/h needs: hold the top of the bracket rather than stall.
            Err(PhysicsError::Solver(_)) => MAX_SOLVABLE_SPEED_MS,
            Err(_) => 0.0,
        };
        let dt_seconds = self.options.tick_interval.as_micros() as f64 / 1_000_000.0;
        self.total_distance_m += self.last_speed_ms * dt_seconds;
        self.elapsed_session_time += self.options.tick_interval;
        self.time_since_last_report += self.options.tick_interval;
        self.elapsed_ticks += 1;

        if self.time_since_last_report >= self.options.report_interval {
            self.time_since_last_report = Duration::ZERO;
            self.pending.push(TrainerEvent::Telemetry(TrainerTelemetry {
                speed_ms: self.last_speed_ms,
                power_watts: self.rider_power_watts,
                cadence_rpm: None,
                total_distance_m: self.total_distance_m,
                elapsed_session_time: self
                    .options
                    .report_elapsed_time
                    .then_some(self.elapsed_session_time),
            }));
        }
    }
}

impl TrainerControl for SimulatedTrainer {
    fn connection_state(&self) -> TrainerConnectionState {
        self.connection_state
    }

    fn connect(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.set_connection_state(TrainerConnectionState::Connecting);
        self.set_connection_state(TrainerConnectionState::Connected);
        Ok(())
    }

    fn set_simulation_parameters(
        &mut self,
        params: SimulationParameters,
        _now: Duration,
    ) -> Result<(), TrainerError> {
        self.params = params;
        self.current_env = params.to_environment_params();
        self.current_rider = params.apply_to(&self.rider);
        Ok(())
    }

    fn start(&mut self, now: Duration) -> Result<(), TrainerError> {
        self.next_tick_at = Some(now + self.options.tick_interval);
        Ok(())
    }

    fn stop(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.next_tick_at = None;
        Ok(())
    }

    fn poll(&mut self, now: Duration) -> Vec<TrainerEvent> {
        while let Some(at) = self.next_tick_at {
            if at > now {
                break;
            }
            self.tick();
            self.next_tick_at = Some(at + self.options.tick_interval);
        }
        std::mem::take(&mut self.pending)
    }

    fn dispose(&mut self) {
        self.next_tick_at = None;
        self.set_connection_state(TrainerConnectionState::Disconnected);
    }

    fn manual_power_source(&mut self) -> Option<&mut dyn ManualPowerSource> {
        Some(self)
    }
}

impl ManualPowerSource for SimulatedTrainer {
    /// Read by the next tick. Negative or non-finite power is treated as zero: there is no
    /// "braking via wattage".
    fn set_rider_power_watts(&mut self, watts: f64) {
        self.rider_power_watts = if watts.is_finite() && watts > 0.0 {
            watts
        } else {
            0.0
        };
    }
}
