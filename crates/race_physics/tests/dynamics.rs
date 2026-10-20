//! Behavioural properties of the integrator, derived from its specification rather than from its
//! output.

use std::time::Duration;

use race_physics::{
    power_for_speed, solve_speed_for_power, step_longitudinal_dynamics, EnvironmentParams,
    LongitudinalDynamicsConfig, LongitudinalStepInput, LongitudinalStepResult, PhysicsError,
    RiderPhysicalParams, SimulationTimestep, SolverOptions, DEFAULT_MAX_PROPULSIVE_FORCE_N,
};

const RIDER: RiderPhysicalParams = RiderPhysicalParams::new(78.0);

fn input(speed: f64, power: f64) -> LongitudinalStepInput {
    LongitudinalStepInput::new(
        speed,
        power,
        RIDER,
        EnvironmentParams::default(),
        SimulationTimestep::RECOMMENDED,
    )
}

fn step(i: LongitudinalStepInput) -> LongitudinalStepResult {
    step_longitudinal_dynamics(&i, &LongitudinalDynamicsConfig::default()).unwrap()
}

#[test]
fn propulsive_force_is_capped_at_rest_and_continuous_at_the_crossover() {
    assert_eq!(
        step(input(0.0, 300.0)).propulsive_force_n,
        DEFAULT_MAX_PROPULSIVE_FORCE_N
    );
    assert_eq!(
        step(input(0.0, 0.0)).propulsive_force_n,
        0.0,
        "zero power is zero force"
    );

    let wheel = 300.0 * RIDER.drive_train_efficiency;
    assert_eq!(step(input(10.0, 300.0)).propulsive_force_n, wheel / 10.0);

    let crossover = wheel / DEFAULT_MAX_PROPULSIVE_FORCE_N;
    let below = step(input(crossover, 300.0)).propulsive_force_n;
    let above = step(input(crossover * (1.0 + 1e-12), 300.0)).propulsive_force_n;
    assert!((below - above).abs() < 1e-6);
}

#[test]
fn net_force_is_exactly_the_sum_of_the_reported_parts() {
    let mut i = input(9.0, 250.0);
    i.environment = EnvironmentParams {
        grade_fraction: 0.03,
        headwind_ms: 2.0,
        ..Default::default()
    };
    i.braking_force_n = 12.0;
    let r = step(i);
    assert_eq!(
        r.net_force_n,
        r.propulsive_force_n
            - r.gravity_force_n
            - r.rolling_resistance_force_n
            - r.applied_aero_drag_force_n
            - r.braking_force_n
    );
    assert_eq!(r.raw_acceleration_ms2, r.net_force_n / RIDER.total_mass_kg);
}

#[test]
fn braking_is_monotonic_independent_of_power_and_never_reverses() {
    let mut previous = f64::INFINITY;
    for braking in [0.0, 10.0, 50.0, 200.0] {
        let mut i = input(8.0, 200.0);
        i.braking_force_n = braking;
        let a = step(i).raw_acceleration_ms2;
        assert!(a < previous);
        previous = a;
    }
    let decel = |power: f64| {
        let mut with = input(8.0, power);
        with.braking_force_n = 60.0;
        step(input(8.0, power)).raw_acceleration_ms2 - step(with).raw_acceleration_ms2
    };
    assert!((decel(0.0) - decel(400.0)).abs() < 1e-12);

    let mut hard = input(0.5, 0.0);
    hard.braking_force_n = 1000.0;
    let r = step(hard);
    assert_eq!(r.next_speed_ms, 0.0);
    assert_eq!(r.distance_delta_m, 0.0);
    assert!(r.zero_speed_clamp_applied);
}

#[test]
fn the_drag_multiplier_changes_only_the_aerodynamic_term() {
    let clean = step(input(11.0, 260.0));
    let mut sheltered_input = input(11.0, 260.0);
    sheltered_input.aerodynamic_drag_multiplier = 0.7;
    let sheltered = step(sheltered_input);
    assert_eq!(sheltered.propulsive_force_n, clean.propulsive_force_n);
    assert_eq!(sheltered.gravity_force_n, clean.gravity_force_n);
    assert_eq!(
        sheltered.rolling_resistance_force_n,
        clean.rolling_resistance_force_n
    );
    assert_eq!(sheltered.raw_aero_drag_force_n, clean.raw_aero_drag_force_n);
    assert_eq!(
        sheltered.applied_aero_drag_force_n,
        0.7 * clean.raw_aero_drag_force_n
    );
    assert!(sheltered.next_speed_ms > clean.next_speed_ms);
}

#[test]
fn a_step_at_the_solver_equilibrium_barely_moves_the_rider() {
    let env = EnvironmentParams::with_grade(0.02);
    let v = solve_speed_for_power(240.0, &RIDER, &env, SolverOptions::default()).unwrap();
    let mut i = input(v, 240.0);
    i.environment = env;
    let r = step(i);
    assert!((r.next_speed_ms - v).abs() < 1e-4);
    assert!((power_for_speed(v, &RIDER, &env) - 240.0).abs() < 0.01);
}

#[test]
fn semi_implicit_euler_with_exactly_one_step() {
    let r = step(input(6.0, 300.0));
    let dt = 0.05;
    assert_eq!(r.next_speed_ms, 6.0 + r.raw_acceleration_ms2 * dt);
    assert_eq!(r.distance_delta_m, r.next_speed_ms * dt);
}

#[test]
fn a_tailwind_faster_than_the_rider_assists_and_a_descent_rolls_away() {
    let mut tail = input(4.0, 0.0);
    tail.environment.headwind_ms = -10.0;
    assert!(step(tail).raw_aero_drag_force_n < 0.0);

    let mut descent = input(0.0, 0.0);
    descent.environment.grade_fraction = -0.08;
    assert!(step(descent).next_speed_ms > 0.0);

    let mut climb = input(0.0, 0.0);
    climb.environment.grade_fraction = 0.08;
    let r = step(climb);
    assert_eq!(r.next_speed_ms, 0.0);
    assert!(r.zero_speed_clamp_applied);
}

#[test]
fn every_invalid_input_is_rejected_by_name_before_computing() {
    type Mutation = fn(&mut LongitudinalStepInput);
    let cases: [(&str, Mutation); 15] = [
        ("speedMs", |i| i.speed_ms = -1.0),
        ("speedMs", |i| i.speed_ms = f64::NAN),
        ("powerWatts", |i| i.power_watts = f64::INFINITY),
        ("brakingForceN", |i| i.braking_force_n = -0.1),
        ("aerodynamicDragMultiplier", |i| {
            i.aerodynamic_drag_multiplier = 0.0
        }),
        ("aerodynamicDragMultiplier", |i| {
            i.aerodynamic_drag_multiplier = 1.01
        }),
        ("timestep", |i| i.timestep = Duration::ZERO),
        ("rider.totalMassKg", |i| i.rider.total_mass_kg = 0.0),
        ("rider.crr", |i| i.rider.crr = -0.001),
        ("rider.cdaM2", |i| i.rider.cda_m2 = f64::NAN),
        ("rider.driveTrainEfficiency", |i| {
            i.rider.drive_train_efficiency = 1.5
        }),
        ("environment.gradeFraction", |i| {
            i.environment.grade_fraction = f64::NAN
        }),
        ("environment.airDensityKgM3", |i| {
            i.environment.air_density_kg_m3 = 0.0
        }),
        ("environment.headwindMs", |i| {
            i.environment.headwind_ms = f64::NEG_INFINITY
        }),
        ("environment.airDensityKgM3", |i| {
            i.environment.air_density_kg_m3 = -1.0
        }),
    ];
    for (expected, mutate) in cases {
        let mut i = input(5.0, 200.0);
        mutate(&mut i);
        match step_longitudinal_dynamics(&i, &LongitudinalDynamicsConfig::default()) {
            Err(PhysicsError::InvalidArgument { name, .. }) => assert_eq!(name, expected),
            other => panic!("{expected}: {other:?}"),
        }
    }
    assert!(LongitudinalDynamicsConfig::new(0.0).is_err());
    assert!(LongitudinalDynamicsConfig::new(f64::NAN).is_err());
    assert_eq!(
        LongitudinalDynamicsConfig::default().max_propulsive_force_n(),
        DEFAULT_MAX_PROPULSIVE_FORCE_N
    );
}

#[test]
fn a_non_finite_result_is_refused() {
    let mut i = input(0.0, 1000.0);
    i.rider.total_mass_kg = f64::MIN_POSITIVE;
    assert!(matches!(
        step_longitudinal_dynamics(&i, &LongitudinalDynamicsConfig::default()),
        Err(PhysicsError::NonFiniteResult { .. })
    ));
}
