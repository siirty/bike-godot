//! The simulated trainer, driven with explicit time.

use std::time::Duration;

use race_physics::{solve_speed_for_power, EnvironmentParams, RiderPhysicalParams, SolverOptions};
use trainer_control::{
    SimulatedTrainer, SimulatedTrainerOptions, SimulationParameters, TrainerConnectionState,
    TrainerControl, TrainerEvent, TrainerTelemetry,
};

const RIDER: RiderPhysicalParams = RiderPhysicalParams::new(80.0);

fn telemetry(events: &[TrainerEvent]) -> Vec<TrainerTelemetry> {
    events
        .iter()
        .filter_map(|e| match e {
            TrainerEvent::Telemetry(t) => Some(*t),
            TrainerEvent::ConnectionState(_) => None,
        })
        .collect()
}

fn running(options: SimulatedTrainerOptions) -> SimulatedTrainer {
    let mut t = SimulatedTrainer::new(RIDER, options);
    t.connect(Duration::ZERO).unwrap();
    t.start(Duration::ZERO).unwrap();
    t
}

#[test]
fn connect_reports_connecting_then_connected_and_loss_is_observable() {
    let mut t = SimulatedTrainer::new(RIDER, SimulatedTrainerOptions::default());
    t.connect(Duration::ZERO).unwrap();
    assert_eq!(
        t.poll(Duration::ZERO),
        [
            TrainerEvent::ConnectionState(TrainerConnectionState::Connecting),
            TrainerEvent::ConnectionState(TrainerConnectionState::Connected),
        ]
    );
    t.simulate_connection_loss();
    assert_eq!(t.connection_state(), TrainerConnectionState::ConnectionLost);
    t.dispose();
    assert_eq!(t.connection_state(), TrainerConnectionState::Disconnected);
}

#[test]
fn telemetry_follows_the_reporting_cadence_not_the_tick_cadence() {
    let mut t = running(SimulatedTrainerOptions::default());
    t.manual_power_source()
        .unwrap()
        .set_rider_power_watts(200.0);
    let reports = telemetry(&t.poll(Duration::from_millis(2_000)));
    assert_eq!(reports.len(), 4, "2 Hz over 2 s");
    assert_eq!(
        reports
            .iter()
            .map(|r| r.elapsed_session_time.unwrap())
            .collect::<Vec<_>>(),
        [500, 1000, 1500, 2000].map(Duration::from_millis)
    );
    assert_eq!(t.debug_state().elapsed_ticks, 40);
}

#[test]
fn speed_is_the_solver_steady_state_and_distance_integrates_it() {
    let mut t = running(SimulatedTrainerOptions::default());
    t.set_simulation_parameters(
        SimulationParameters::default().with_grade(0.03),
        Duration::ZERO,
    )
    .unwrap();
    t.manual_power_source()
        .unwrap()
        .set_rider_power_watts(250.0);
    let reports = telemetry(&t.poll(Duration::from_secs(10)));
    let expected = solve_speed_for_power(
        250.0,
        &RIDER,
        &EnvironmentParams::with_grade(0.03),
        SolverOptions::default(),
    )
    .unwrap();
    let last = reports.last().unwrap();
    assert_eq!(last.speed_ms, expected);
    assert!((last.total_distance_m - expected * 10.0).abs() < 1e-9);
    assert_eq!(last.power_watts, 250.0);
}

#[test]
fn zero_power_holds_still_and_stop_freezes_the_session() {
    let mut t = running(SimulatedTrainerOptions::default());
    let reports = telemetry(&t.poll(Duration::from_secs(3)));
    assert!(reports
        .iter()
        .all(|r| r.speed_ms == 0.0 && r.total_distance_m == 0.0));

    t.manual_power_source()
        .unwrap()
        .set_rider_power_watts(200.0);
    t.poll(Duration::from_secs(4));
    t.stop(Duration::from_secs(4)).unwrap();
    let before = t.debug_state();
    assert!(telemetry(&t.poll(Duration::from_secs(60))).is_empty());
    assert_eq!(t.debug_state(), before);

    // Resuming continues the session; it resets nothing.
    t.start(Duration::from_secs(60)).unwrap();
    let resumed = telemetry(&t.poll(Duration::from_millis(60_500)));
    assert_eq!(
        resumed[0].elapsed_session_time,
        Some(Duration::from_millis(4_500))
    );
}

#[test]
fn elapsed_time_can_be_omitted_like_real_trainers_that_never_set_the_flag() {
    let mut t = running(SimulatedTrainerOptions {
        report_elapsed_time: false,
        ..SimulatedTrainerOptions::default()
    });
    let reports = telemetry(&t.poll(Duration::from_secs(1)));
    assert!(!reports.is_empty());
    assert!(reports.iter().all(|r| r.elapsed_session_time.is_none()));
}

#[test]
fn sim_writes_apply_crr_and_cda_over_the_rider_and_nothing_else() {
    let params = SimulationParameters {
        grade_fraction: 0.01,
        headwind_ms: 2.0,
        crr: 0.007,
        cda_m2: 0.25,
    };
    let applied = params.apply_to(&RIDER);
    assert_eq!(applied.total_mass_kg, RIDER.total_mass_kg);
    assert_eq!(applied.drive_train_efficiency, RIDER.drive_train_efficiency);
    assert_eq!((applied.crr, applied.cda_m2), (0.007, 0.25));
    assert_eq!(
        SimulationParameters::from_physics(&applied, &params.to_environment_params()),
        params
    );
}

#[test]
fn unsanitary_manual_power_is_treated_as_zero() {
    let mut t = running(SimulatedTrainerOptions::default());
    t.manual_power_source()
        .unwrap()
        .set_rider_power_watts(f64::NAN);
    assert_eq!(t.debug_state().rider_power_w, 0.0);
    t.manual_power_source()
        .unwrap()
        .set_rider_power_watts(-50.0);
    assert_eq!(t.debug_state().rider_power_w, 0.0);
}
