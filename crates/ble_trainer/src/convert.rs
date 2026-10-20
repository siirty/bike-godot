//! The one place app-side values become `trainer_core` quantities, and back.

use std::time::Duration;

use trainer_control::{SimulationParameters, TrainerTelemetry};
use trainer_core::units::{DragArea, GradeFraction};
use trainer_core::SimParams;

pub(crate) fn sim_params(p: &SimulationParameters) -> SimParams {
    SimParams {
        grade: GradeFraction::from_fraction(p.grade_fraction),
        wind_speed_mps: p.headwind_ms,
        crr: p.crr,
        cda: DragArea::from_m2(p.cda_m2),
    }
}

pub(crate) fn telemetry(t: &trainer_core::TrainerTelemetry) -> TrainerTelemetry {
    TrainerTelemetry {
        speed_ms: t.speed_mps,
        power_watts: t.power_w,
        cadence_rpm: t.cadence_rpm,
        total_distance_m: t.total_distance_m,
        // FTMS Elapsed Time is optional and whole seconds. Absent stays absent: a stand-in zero
        // would snap the ride clock back on every report.
        elapsed_session_time: t.elapsed_time_s.map(|s| Duration::from_secs(u64::from(s))),
    }
}
