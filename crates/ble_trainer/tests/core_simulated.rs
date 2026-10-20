//! The core simulator under explicit time.

use std::time::Duration;

use ble_trainer::{CoreSimulatedTrainer, SIM_REPORT_INTERVAL};
use trainer_control::{
    SimulationParameters, TrainerConnectionState, TrainerControl, TrainerEvent, TrainerTelemetry,
};

fn telemetry(events: &[TrainerEvent]) -> Vec<TrainerTelemetry> {
    events
        .iter()
        .filter_map(|e| match e {
            TrainerEvent::Telemetry(t) => Some(*t),
            TrainerEvent::ConnectionState(_) => None,
        })
        .collect()
}

fn started() -> CoreSimulatedTrainer {
    let mut t = CoreSimulatedTrainer::new(80.0, 0.004, 0.32);
    t.connect(Duration::ZERO).unwrap();
    t.start(Duration::ZERO).unwrap();
    t
}

#[test]
fn reports_once_per_report_interval_of_explicit_time() {
    let mut t = started();
    let events = t.poll(Duration::ZERO);
    assert_eq!(
        events,
        vec![TrainerEvent::ConnectionState(
            TrainerConnectionState::Connected
        )]
    );
    assert!(t
        .poll(SIM_REPORT_INTERVAL - Duration::from_millis(1))
        .is_empty());
    assert_eq!(telemetry(&t.poll(SIM_REPORT_INTERVAL)).len(), 1);
    // A stalled frame catches up rather than drifting.
    assert_eq!(telemetry(&t.poll(SIM_REPORT_INTERVAL * 11)).len(), 10);
}

#[test]
fn power_accelerates_the_rider_with_momentum_and_a_climb_slows_them() {
    let mut t = started();
    t.manual_power_source()
        .unwrap()
        .set_rider_power_watts(250.0);
    let early = *telemetry(&t.poll(SIM_REPORT_INTERVAL)).last().unwrap();
    let later = *telemetry(&t.poll(Duration::from_secs(60))).last().unwrap();
    assert!(
        early.speed_ms > 0.0 && early.speed_ms < later.speed_ms,
        "speed builds over time"
    );
    assert_eq!(later.power_watts, 250.0);
    assert!(later.total_distance_m > 300.0);

    t.set_simulation_parameters(
        SimulationParameters {
            grade_fraction: 0.08,
            ..SimulationParameters::default()
        },
        Duration::from_secs(60),
    )
    .unwrap();
    let climbing = *telemetry(&t.poll(Duration::from_secs(120))).last().unwrap();
    assert!(
        climbing.speed_ms < later.speed_ms / 2.0,
        "{} vs {}",
        climbing.speed_ms,
        later.speed_ms
    );
    assert_eq!(t.debug_snapshot().params.grade_fraction, 0.08);
}

#[test]
fn stopping_ends_reports_and_bad_slider_values_are_zero() {
    let mut t = started();
    t.manual_power_source()
        .unwrap()
        .set_rider_power_watts(f64::NAN);
    assert_eq!(t.debug_snapshot().rider_power_w, 0.0);
    t.stop(Duration::from_secs(1)).unwrap();
    assert!(telemetry(&t.poll(Duration::from_secs(10))).is_empty());
    assert!(!t.debug_snapshot().running);
}

#[test]
fn writes_before_connect_are_refused_and_dispose_disconnects() {
    let mut t = CoreSimulatedTrainer::new(80.0, 0.004, 0.32);
    assert!(t.start(Duration::ZERO).is_err());
    t.connect(Duration::ZERO).unwrap();
    t.dispose();
    assert_eq!(t.connection_state(), TrainerConnectionState::Disconnected);
}
