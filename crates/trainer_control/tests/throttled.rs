//! The SIM-write policy, driven with explicit time. Expectations come from the policy text
//! (".claude/rules/trainer-control-ble.md": ~0.2 % grade delta, max ~1 Hz), not from the code.

use std::time::Duration;

use trainer_control::{
    RecordingTrainerControl, SimulationParameters, ThrottledTrainerControl, TrainerConnectionState,
    TrainerControl, TrainerEvent, TrainerTelemetry,
};

fn ms(v: u64) -> Duration {
    Duration::from_millis(v)
}

fn grade(g: f64) -> SimulationParameters {
    SimulationParameters::default().with_grade(g)
}

fn throttled() -> ThrottledTrainerControl<RecordingTrainerControl> {
    ThrottledTrainerControl::new(RecordingTrainerControl::new())
}

fn written(t: &ThrottledTrainerControl<RecordingTrainerControl>) -> Vec<f64> {
    t.inner().grades_written()
}

#[test]
fn the_first_write_always_lands() {
    let mut t = throttled();
    t.set_simulation_parameters(grade(0.0), ms(0)).unwrap();
    assert_eq!(written(&t), [0.0]);
}

#[test]
fn a_material_change_inside_the_window_waits_and_is_flushed_when_it_opens() {
    let mut t = throttled();
    t.set_simulation_parameters(grade(0.0), ms(0)).unwrap();
    t.set_simulation_parameters(grade(0.05), ms(300)).unwrap();
    assert_eq!(written(&t), [0.0]);
    t.poll(ms(999));
    assert_eq!(written(&t), [0.0]);
    t.poll(ms(1000));
    assert_eq!(written(&t), [0.0, 0.05]);
}

#[test]
fn a_material_change_after_the_window_writes_immediately() {
    let mut t = throttled();
    t.set_simulation_parameters(grade(0.0), ms(0)).unwrap();
    t.set_simulation_parameters(grade(0.05), ms(1000)).unwrap();
    assert_eq!(written(&t), [0.0, 0.05]);
}

#[test]
fn only_the_newest_value_of_a_burst_survives() {
    let mut t = throttled();
    t.set_simulation_parameters(grade(0.0), ms(0)).unwrap();
    for (i, g) in [0.01, 0.02, 0.03, 0.04].into_iter().enumerate() {
        t.set_simulation_parameters(grade(g), ms(100 * (i as u64 + 1)))
            .unwrap();
    }
    t.poll(ms(1000));
    t.poll(ms(5000));
    assert_eq!(written(&t), [0.0, 0.04]);
}

#[test]
fn writes_settle_near_one_hertz_over_a_long_ride() {
    let mut t = throttled();
    // A 30 fps caller on a steadily rising grade for 60 s.
    for frame in 0..1800u64 {
        let now = Duration::from_micros(frame * 33_333);
        t.poll(now);
        t.set_simulation_parameters(grade(frame as f64 * 0.0005), now)
            .unwrap();
    }
    let writes = written(&t).len();
    assert!((55..=62).contains(&writes), "{writes} writes in 60 s");
}

#[test]
fn materiality_is_against_the_last_written_grade_and_inclusive_at_the_threshold() {
    let mut t = throttled();
    t.set_simulation_parameters(grade(0.0), ms(0)).unwrap();
    t.set_simulation_parameters(grade(0.0019), ms(5000))
        .unwrap();
    assert_eq!(written(&t), [0.0], "sub-threshold is dropped");
    t.poll(ms(9000));
    assert_eq!(written(&t), [0.0], "and never flushed");

    t.set_simulation_parameters(grade(0.002), ms(10_000))
        .unwrap();
    assert_eq!(
        written(&t),
        [0.0, 0.002],
        "exactly the threshold is material"
    );

    // Creeping by sub-threshold steps still adds up against the last *written* grade.
    let mut now = 20_000;
    for g in [0.003, 0.0035, 0.004] {
        now += 2000;
        t.set_simulation_parameters(grade(g), ms(now)).unwrap();
    }
    assert_eq!(written(&t), [0.0, 0.002, 0.004]);
}

#[test]
fn non_grade_changes_always_get_through_and_identical_repeats_do_not() {
    let mut t = throttled();
    t.set_simulation_parameters(grade(0.01), ms(0)).unwrap();
    let windy = SimulationParameters {
        headwind_ms: 3.0,
        ..grade(0.01)
    };
    t.set_simulation_parameters(windy, ms(10)).unwrap();
    let crr = SimulationParameters {
        crr: 0.006,
        ..windy
    };
    t.set_simulation_parameters(crr, ms(20)).unwrap();
    let cda = SimulationParameters {
        cda_m2: 0.28,
        ..crr
    };
    t.set_simulation_parameters(cda, ms(30)).unwrap();
    t.set_simulation_parameters(cda, ms(5000)).unwrap();
    assert_eq!(t.inner().sim_param_writes.len(), 4);
    assert_eq!(t.last_written_parameters(), Some(cda));
}

#[test]
fn a_non_grade_write_supersedes_a_pending_grade() {
    let mut t = throttled();
    t.set_simulation_parameters(grade(0.0), ms(0)).unwrap();
    t.set_simulation_parameters(grade(0.05), ms(100)).unwrap();
    let windy = SimulationParameters {
        headwind_ms: 2.0,
        ..grade(0.0)
    };
    t.set_simulation_parameters(windy, ms(200)).unwrap();
    t.poll(ms(3000));
    assert_eq!(written(&t), [0.0, 0.0]);
}

#[test]
fn everything_else_delegates_and_dispose_drops_a_pending_flush() {
    let mut t = throttled();
    t.connect(ms(0)).unwrap();
    t.start(ms(0)).unwrap();
    t.stop(ms(0)).unwrap();
    assert_eq!(t.connection_state(), TrainerConnectionState::Connected);
    let report = TrainerTelemetry {
        speed_ms: 9.0,
        power_watts: 200.0,
        cadence_rpm: Some(88.0),
        total_distance_m: 12.0,
        elapsed_session_time: None,
    };
    t.inner_mut().emit_telemetry(report);
    assert_eq!(
        t.poll(ms(1)),
        [
            TrainerEvent::ConnectionState(TrainerConnectionState::Connected),
            TrainerEvent::Telemetry(report)
        ]
    );
    assert!(
        t.manual_power_source().is_none(),
        "a recorder stands in for a real trainer"
    );

    t.set_simulation_parameters(grade(0.0), ms(10)).unwrap();
    t.set_simulation_parameters(grade(0.05), ms(20)).unwrap();
    t.dispose();
    t.poll(ms(5000));
    assert_eq!(written(&t), [0.0]);
    let inner = t.inner();
    assert_eq!(
        (
            inner.connect_count,
            inner.start_count,
            inner.stop_count,
            inner.dispose_count
        ),
        (1, 1, 1, 1)
    );
}
