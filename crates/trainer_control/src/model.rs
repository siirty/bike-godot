use std::time::Duration;

use race_physics::{EnvironmentParams, PhysicsDefaults, RiderPhysicalParams};

/// What FTMS "Set Indoor Bike Simulation Parameters" (opcode `0x11`) carries: a projection of the
/// physics types onto the wire shape, not a parallel model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulationParameters {
    /// Grade as rise/run.
    pub grade_fraction: f64,
    /// Headwind, in m/s. Negative is a tailwind.
    pub headwind_ms: f64,
    /// Coefficient of rolling resistance.
    pub crr: f64,
    /// Drag area, in m². The FTMS "Cw" conversion happens at the wire boundary.
    pub cda_m2: f64,
}

impl Default for SimulationParameters {
    fn default() -> Self {
        Self {
            grade_fraction: 0.0,
            headwind_ms: 0.0,
            crr: PhysicsDefaults::CRR,
            cda_m2: PhysicsDefaults::CDA_M2,
        }
    }
}

impl SimulationParameters {
    /// The whole mapping: grade and wind from the environment, Crr and CdA from the rider. Air
    /// density, mass and drivetrain efficiency have no FTMS field.
    pub fn from_physics(rider: &RiderPhysicalParams, env: &EnvironmentParams) -> Self {
        Self {
            grade_fraction: env.grade_fraction,
            headwind_ms: env.headwind_ms,
            crr: rider.crr,
            cda_m2: rider.cda_m2,
        }
    }

    /// The environment half, with air density from [`PhysicsDefaults`] (FTMS does not carry it).
    pub fn to_environment_params(&self) -> EnvironmentParams {
        EnvironmentParams {
            grade_fraction: self.grade_fraction,
            headwind_ms: self.headwind_ms,
            ..EnvironmentParams::default()
        }
    }

    /// `rider` with these Crr and CdA, every other field intact.
    pub fn apply_to(&self, rider: &RiderPhysicalParams) -> RiderPhysicalParams {
        RiderPhysicalParams {
            crr: self.crr,
            cda_m2: self.cda_m2,
            ..*rider
        }
    }

    /// The same parameters on a different grade.
    pub fn with_grade(self, grade_fraction: f64) -> Self {
        Self {
            grade_fraction,
            ..self
        }
    }
}

/// One telemetry report: the fields of FTMS "Indoor Bike Data" (`0x2AD2`). Everything here is
/// something a real trainer reports. Only [`crate::SimulatedTrainer`] may derive speed from power.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrainerTelemetry {
    /// The trainer's own SIM-computed speed, in m/s.
    pub speed_ms: f64,
    /// Instantaneous rider power, in watts.
    pub power_watts: f64,
    /// Instantaneous cadence, in rpm, if the trainer reports it.
    pub cadence_rpm: Option<f64>,
    /// Total distance this session, in metres.
    pub total_distance_m: f64,
    /// Elapsed session time, if the trainer reports it. Absent is not zero: an absent field read
    /// as zero would reset a ride clock several times a second.
    pub elapsed_session_time: Option<Duration>,
}

/// The state of the link between the app and a trainer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrainerConnectionState {
    /// Not connected, never attempted, or disposed.
    Disconnected,
    /// A connect is in flight.
    Connecting,
    /// Telemetry can be expected and writes should land.
    Connected,
    /// The link dropped without being asked to. What a UI surfaces mid-ride.
    ConnectionLost,
}

/// Something that happened on a trainer since the last poll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrainerEvent {
    /// A telemetry report.
    Telemetry(TrainerTelemetry),
    /// A connection transition.
    ConnectionState(TrainerConnectionState),
}

/// A trainer operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct TrainerError {
    /// What went wrong.
    pub message: String,
}

impl TrainerError {
    /// An error with `message`.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// An inspectable snapshot of a simulated trainer's internal state, for the SIM debug screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimDebugState {
    /// The rider power the sim was last given, in watts.
    pub rider_power_w: f64,
    /// Speed after the last tick, in m/s.
    pub speed_mps: f64,
    /// Accumulated distance, in metres.
    pub total_distance_m: f64,
    /// Physics ticks advanced.
    pub elapsed_ticks: u64,
    /// The tick interval, in milliseconds.
    pub tick_interval_ms: u64,
    /// Ticks per telemetry report.
    pub report_every_ticks: u64,
    /// Started and not stopped.
    pub running: bool,
    /// The current SIM parameters.
    pub params: SimulationParameters,
}
