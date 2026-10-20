//! Parity against vectors recorded from the Dart prototype (`fixtures/golden/race_physics.json`).
//! Every double is compared bit-for-bit.

use std::path::PathBuf;
use std::time::Duration;

use race_physics::convergence::{
    convergence_corpus, render_convergence_report, run_track, COMPARISON_GRID,
};
use race_physics::{
    aero_drag_force_n, gravity_force_n, power_for_speed, rolling_resistance_force_n,
    solve_grade_for_power, solve_speed_for_power, step_longitudinal_dynamics, EnvironmentParams,
    LongitudinalDynamicsConfig, LongitudinalStepInput, PhysicsError, RiderPhysicalParams,
    SimulationTimestep, SolverOptions,
};
use serde_json::Value;

fn golden() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden/race_physics.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("read vectors")).expect("parse")
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn rider(v: &Value) -> RiderPhysicalParams {
    serde_json::from_value(v.clone()).expect("rider JSON")
}

#[track_caller]
fn same(actual: f64, expected: &Value, context: &str) {
    let expected = f(expected);
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{context}: rust {actual:e} vs fixture {expected:e}"
    );
}

fn error_kind(e: &PhysicsError) -> &'static str {
    match e {
        PhysicsError::Solver(_) => "solver",
        PhysicsError::InvalidArgument { .. } => "argument",
        PhysicsError::NonFiniteResult { .. } => "state",
    }
}

#[test]
fn rider_json_round_trips_in_the_prototype_shape() {
    let g = golden();
    let json = &g["powerForSpeed"][0]["rider"];
    let params = rider(json);
    assert_eq!(serde_json::to_value(params).unwrap(), *json);
    let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
    let encoded = serde_json::to_string(&params).unwrap();
    // Field order is part of the snapshot byte format.
    assert!(
        encoded.starts_with("{\"totalMassKg\":"),
        "{encoded} / {keys:?}"
    );
}

#[test]
fn forces_match() {
    let g = golden();
    for c in g["forces"].as_array().unwrap() {
        let ctx = c.to_string();
        same(
            gravity_force_n(f(&c["massKg"]), f(&c["grade"])),
            &c["gravityN"],
            &ctx,
        );
        same(
            rolling_resistance_force_n(f(&c["massKg"]), f(&c["grade"]), 0.0053),
            &c["rollingN"],
            &ctx,
        );
    }
    for c in g["aero"].as_array().unwrap() {
        same(
            aero_drag_force_n(
                f(&c["rho"]),
                f(&c["cda"]),
                f(&c["speedMs"]),
                f(&c["headwindMs"]),
            ),
            &c["aeroN"],
            &c.to_string(),
        );
    }
}

fn env(c: &Value) -> EnvironmentParams {
    EnvironmentParams {
        grade_fraction: f(&c["grade"]),
        headwind_ms: f(&c["headwindMs"]),
        ..EnvironmentParams::default()
    }
}

#[test]
fn power_for_speed_matches() {
    let g = golden();
    for c in g["powerForSpeed"].as_array().unwrap() {
        same(
            power_for_speed(f(&c["speedMs"]), &rider(&c["rider"]), &env(c)),
            &c["powerW"],
            &c.to_string(),
        );
    }
}

#[test]
fn speed_solver_matches_including_failures() {
    let g = golden();
    let cases = g["solveSpeedForPower"].as_array().unwrap();
    assert!(
        cases.iter().any(|c| c.get("error").is_some()),
        "vectors cover failures"
    );
    for c in cases {
        let ctx = c.to_string();
        let result = solve_speed_for_power(
            f(&c["powerW"]),
            &rider(&c["rider"]),
            &env(c),
            SolverOptions::default(),
        );
        match c.get("error") {
            Some(kind) => assert_eq!(error_kind(&result.unwrap_err()), kind, "{ctx}"),
            None => same(result.unwrap(), &c["speedMs"], &ctx),
        }
    }
}

#[test]
fn grade_solver_matches_including_failures() {
    let g = golden();
    for c in g["solveGradeForPower"].as_array().unwrap() {
        let ctx = c.to_string();
        let result = solve_grade_for_power(
            f(&c["powerW"]),
            f(&c["speedMs"]),
            &rider(&c["rider"]),
            &EnvironmentParams::default(),
            SolverOptions::default(),
        );
        match c.get("error") {
            Some(kind) => assert_eq!(error_kind(&result.unwrap_err()), kind, "{ctx}"),
            None => same(result.unwrap(), &c["grade"], &ctx),
        }
    }
}

#[test]
fn single_steps_match_field_by_field() {
    let g = golden();
    for c in g["steps"].as_array().unwrap() {
        let ctx = c.to_string();
        let input = LongitudinalStepInput {
            speed_ms: f(&c["speedMs"]),
            power_watts: f(&c["powerW"]),
            rider: rider(&c["rider"]),
            environment: env(c),
            timestep: Duration::from_micros(c["stepMicros"].as_u64().unwrap()),
            braking_force_n: f(&c["brakingN"]),
            aerodynamic_drag_multiplier: f(&c["dragMultiplier"]),
        };
        let config = LongitudinalDynamicsConfig::new(f(&c["maxPropulsiveForceN"])).unwrap();
        let r = step_longitudinal_dynamics(&input, &config).unwrap();
        same(r.next_speed_ms, &c["nextSpeedMs"], &ctx);
        same(r.distance_delta_m, &c["distanceDeltaM"], &ctx);
        same(r.propulsive_force_n, &c["propulsiveForceN"], &ctx);
        same(r.gravity_force_n, &c["gravityForceN"], &ctx);
        same(
            r.rolling_resistance_force_n,
            &c["rollingResistanceForceN"],
            &ctx,
        );
        same(r.raw_aero_drag_force_n, &c["rawAeroDragForceN"], &ctx);
        same(
            r.applied_aero_drag_force_n,
            &c["appliedAeroDragForceN"],
            &ctx,
        );
        same(r.net_force_n, &c["netForceN"], &ctx);
        same(r.raw_acceleration_ms2, &c["rawAccelerationMs2"], &ctx);
        assert_eq!(
            r.zero_speed_clamp_applied,
            c["zeroSpeedClampApplied"].as_bool().unwrap(),
            "{ctx}"
        );
    }
}

#[test]
fn corpus_trajectories_match_at_the_recommended_step() {
    let g = golden();
    let expected = g["trajectories50ms"].as_array().unwrap();
    let config = LongitudinalDynamicsConfig::default();
    let mut i = 0;
    for scenario in convergence_corpus() {
        for track in &scenario.tracks {
            let e = &expected[i];
            i += 1;
            assert_eq!(e["scenario"], scenario.name);
            assert_eq!(e["track"], track.name);
            let t = run_track(
                track,
                SimulationTimestep::RECOMMENDED,
                &config,
                COMPARISON_GRID,
            )
            .unwrap();
            assert_eq!(
                t.times_micros.len() as u64,
                e["sampleCount"].as_u64().unwrap()
            );
            let per_second = (1_000_000 / t.sample_interval_micros) as usize;
            for (k, sample) in e["samples"].as_array().unwrap().iter().enumerate() {
                let idx = k * per_second;
                let ctx = format!("{}/{} sample {idx}", scenario.name, track.name);
                assert_eq!(t.times_micros[idx], sample[0].as_i64().unwrap(), "{ctx}");
                same(t.speed_ms[idx], &sample[1], &ctx);
                same(t.distance_m[idx], &sample[2], &ctx);
            }
            same(t.final_speed_ms(), &e["finalSpeedMs"], scenario.name);
            same(t.final_distance_m(), &e["finalDistanceM"], scenario.name);
            assert_eq!(
                t.stopped_at.map(|d| d.as_micros() as i64),
                e["stoppedAtMicros"].as_i64(),
                "{}",
                scenario.name
            );
            assert_eq!(
                t.stopped_at_distance_m.map(f64::to_bits),
                e["stoppedAtDistanceM"].as_f64().map(f64::to_bits),
                "{}",
                scenario.name
            );
        }
    }
    assert_eq!(i, expected.len());
}

#[test]
fn convergence_report_is_byte_identical() {
    let g = golden();
    assert_eq!(
        render_convergence_report().unwrap(),
        g["convergenceReport"].as_str().unwrap()
    );
}
