//! The fixed-step error budgets. Deliberately tight: they cannot move without the integrator, the
//! corpus or the selected step moving, and then the README table and the budget handed to drafting
//! must be regenerated deliberately.

use std::time::Duration;

use race_physics::convergence::{
    compare_scenario, convergence_corpus, run_track, ScenarioError, CANDIDATE_STEPS,
    COMPARISON_GRID, CONVERGENCE_ALL_ROUNDER, REFERENCE_STEP,
};
use race_physics::{
    solve_speed_for_power, EnvironmentParams, LongitudinalDynamicsConfig, SimulationTimestep,
    SolverOptions,
};

fn config() -> LongitudinalDynamicsConfig {
    LongitudinalDynamicsConfig::default()
}

fn at_recommended() -> Vec<ScenarioError> {
    convergence_corpus()
        .iter()
        .map(|s| {
            compare_scenario(
                s,
                SimulationTimestep::RECOMMENDED,
                REFERENCE_STEP,
                &config(),
                COMPARISON_GRID,
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn corpus_is_simulable_identically_at_every_candidate_step() {
    let mut steps = CANDIDATE_STEPS.to_vec();
    steps.extend([
        REFERENCE_STEP,
        COMPARISON_GRID,
        SimulationTimestep::RECOMMENDED,
    ]);
    for scenario in convergence_corpus() {
        scenario.assert_well_formed(&steps).unwrap();
    }
    for candidate in CANDIDATE_STEPS {
        assert!(candidate >= REFERENCE_STEP * 10);
    }
    assert!(CANDIDATE_STEPS.contains(&SimulationTimestep::RECOMMENDED));
}

#[test]
fn the_selected_step_holds_every_budget() {
    let errors = at_recommended();
    for e in &errors {
        assert!(
            e.max_speed_error_ms() <= SimulationTimestep::SPEED_ERROR_BUDGET_MS,
            "{}",
            e.scenario
        );
        assert!(
            e.max_distance_error_m() <= SimulationTimestep::DISTANCE_ERROR_BUDGET_M,
            "{}",
            e.scenario
        );
        for t in &e.tracks {
            assert!(t.final_distance_error_m <= SimulationTimestep::DISTANCE_ERROR_BUDGET_M);
        }
    }

    let paired: Vec<_> = errors.iter().filter_map(|e| e.max_gap_error_m).collect();
    assert!(!paired.is_empty(), "the corpus must measure a gap at all");
    for gap in paired {
        assert!(gap <= SimulationTimestep::RIDER_GAP_ERROR_BUDGET_M);
    }

    let stopping: Vec<_> = errors
        .iter()
        .flat_map(|e| &e.tracks)
        .filter(|t| t.stopping_time_error.is_some())
        .collect();
    assert!(
        !stopping.is_empty(),
        "the corpus must bring a rider to a stop"
    );
    for t in stopping {
        assert!(t.stopping_time_error.unwrap() <= SimulationTimestep::STOPPING_TIME_ERROR_BUDGET);
        assert!(
            t.stopping_distance_error_m.unwrap()
                <= SimulationTimestep::STOPPING_DISTANCE_ERROR_BUDGET_M
        );
    }
}

#[test]
fn halving_the_step_reduces_the_error_at_first_order() {
    for scenario in convergence_corpus() {
        let mut previous = (f64::INFINITY, f64::INFINITY);
        for step in CANDIDATE_STEPS {
            let e = compare_scenario(&scenario, step, REFERENCE_STEP, &config(), COMPARISON_GRID)
                .unwrap();
            assert!(
                e.max_speed_error_ms() < previous.0,
                "{} at {step:?}",
                scenario.name
            );
            assert!(
                e.max_distance_error_m() < previous.1,
                "{} at {step:?}",
                scenario.name
            );
            previous = (e.max_speed_error_ms(), e.max_distance_error_m());
        }
    }

    let corpus = convergence_corpus();
    let accel = corpus
        .iter()
        .find(|s| s.name == "accelerate-from-rest")
        .unwrap();
    let coarse = compare_scenario(
        accel,
        Duration::from_millis(200),
        REFERENCE_STEP,
        &config(),
        COMPARISON_GRID,
    )
    .unwrap();
    let fine = compare_scenario(
        accel,
        Duration::from_millis(100),
        REFERENCE_STEP,
        &config(),
        COMPARISON_GRID,
    )
    .unwrap();
    let ratio = coarse.max_distance_error_m() / fine.max_distance_error_m();
    assert!((ratio - 2.0).abs() <= 0.3, "first-order ratio was {ratio}");
}

#[test]
fn reruns_are_identical_and_the_reference_is_only_a_measuring_stick() {
    for scenario in convergence_corpus() {
        for track in &scenario.tracks {
            for step in CANDIDATE_STEPS {
                let a = run_track(track, step, &config(), COMPARISON_GRID).unwrap();
                let b = run_track(track, step, &config(), COMPARISON_GRID).unwrap();
                assert_eq!(a, b, "{}", scenario.name);
            }
        }
    }

    let corpus = convergence_corpus();
    let transition = corpus
        .iter()
        .find(|s| s.name == "grade-transition")
        .unwrap();
    let fine = compare_scenario(
        transition,
        SimulationTimestep::RECOMMENDED,
        Duration::from_millis(2),
        &config(),
        COMPARISON_GRID,
    )
    .unwrap();
    let finer = compare_scenario(
        transition,
        SimulationTimestep::RECOMMENDED,
        Duration::from_millis(1),
        &config(),
        COMPARISON_GRID,
    )
    .unwrap();
    assert!((fine.max_distance_error_m() - finer.max_distance_error_m()).abs() <= 0.05);
}

#[test]
fn an_hour_stays_finite_forward_only_and_settles_on_the_solver_equilibrium() {
    let corpus = convergence_corpus();
    let hour = corpus
        .iter()
        .find(|s| s.name == "long-steady-hour")
        .unwrap();
    let t = run_track(
        &hour.tracks[0],
        SimulationTimestep::RECOMMENDED,
        &config(),
        COMPARISON_GRID,
    )
    .unwrap();
    for i in 0..t.speed_ms.len() {
        assert!(
            t.speed_ms[i].is_finite() && t.speed_ms[i] >= 0.0,
            "sample {i}"
        );
        assert!(t.distance_m[i].is_finite(), "sample {i}");
        if i > 0 {
            assert!(
                t.distance_m[i] >= t.distance_m[i - 1],
                "went backwards at {i}"
            );
        }
    }
    let equilibrium = solve_speed_for_power(
        220.0,
        &CONVERGENCE_ALL_ROUNDER,
        &EnvironmentParams::default(),
        SolverOptions::default(),
    )
    .unwrap();
    assert!((t.final_speed_ms() - equilibrium).abs() <= 1e-3);
}
