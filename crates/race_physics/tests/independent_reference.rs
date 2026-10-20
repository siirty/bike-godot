//! Reference values computed independently of any implementation, from the documented formula
//! `P(v) = [m·g·(sinθ + Crr·cosθ) + 0.5·ρ·CdA·(v+w)²]·v/η` for m = 80 kg, Crr = 0.005,
//! CdA = 0.32, ρ = 1.225, η = 0.975 (carried over from the prototype's golden solver test).

use race_physics::{
    power_for_speed, solve_speed_for_power, EnvironmentParams, RiderPhysicalParams, SolverOptions,
};

const CASES: [(f64, f64, f64); 6] = [
    (8.0, 0.0, 135.1111),
    (10.0, 0.0, 241.2581),
    (12.0, 0.0, 395.6512),
    (8.0, 0.06, 520.5911),
    (6.0, 0.06, 356.671),
    (15.0, -0.03, 376.8542),
];

#[test]
fn forward_and_inverse_match_the_independent_reference() {
    let rider = RiderPhysicalParams::new(80.0);
    for (speed, grade, watts) in CASES {
        let env = EnvironmentParams::with_grade(grade);
        let p = power_for_speed(speed, &rider, &env);
        assert!(
            (p - watts).abs() < 0.01,
            "P({speed}, {grade}) = {p}, expected {watts}"
        );
        let v = solve_speed_for_power(watts, &rider, &env, SolverOptions::default()).unwrap();
        assert!(
            (v - speed).abs() < 0.01,
            "v({watts} W, {grade}) = {v}, expected {speed}"
        );
    }
}
