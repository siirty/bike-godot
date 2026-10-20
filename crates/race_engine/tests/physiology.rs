//! Regression tests for capability and geometry invariants.
use race_engine::*;
use race_physics::{
    step_longitudinal_dynamics, step_physiology, EnvironmentParams, LongitudinalDynamicsConfig,
    LongitudinalStepInput, PhysiologyParams, RiderPhysicalParams,
};
use std::time::Duration;
fn setup(balance: f64) -> (RaceSimulation, RaceState, RaceInputFrame) {
    let id = RiderId::new("a").unwrap();
    let p = PhysiologyParams::new(250.0, 20_000.0, 1250.0).unwrap();
    let d = RiderDefinition::new(id.clone(), RiderPhysicalParams::new(75.0))
        .unwrap()
        .with_physiology(p);
    let c = RaceSimulationConfig::new(RaceSimulationConfigSpec::new(
        Duration::from_millis(50),
        vec![d],
        SimulationTerrain::flat(0.0).unwrap(),
        LongitudinalDynamicsConfig::default(),
        None,
    ))
    .unwrap();
    let s = RaceState::new(
        Duration::ZERO,
        Pcg32::seeded(1, 1).state(),
        vec![RiderState::new(id.clone(), 0.0, 10.0, 0, None)
            .unwrap()
            .with_w_prime_balance(balance)
            .unwrap()],
    )
    .unwrap();
    let f = RaceInputFrame::new(vec![RiderControlInput::power(id, 5000.0).unwrap()]).unwrap();
    (RaceSimulation::new(c).unwrap(), s, f)
}
#[test]
fn motion_uses_available_power_and_reserve_survives_target_retention() {
    let (sim, s, f) = setup(0.0);
    let next = sim
        .step(&s, &f, sim.config().fixed_step())
        .unwrap()
        .next_state;
    let expected = step_longitudinal_dynamics(
        &LongitudinalStepInput::new(
            10.0,
            250.0,
            RiderPhysicalParams::new(75.0),
            EnvironmentParams::default(),
            Duration::from_millis(50),
        ),
        sim.config().dynamics(),
    )
    .unwrap();
    assert_eq!(next.riders()[0].speed_ms(), expected.next_speed_ms);
    assert_eq!(next.riders()[0].w_prime_balance_j(), Some(0.0));
    let (sim, s, f) = setup(1000.0);
    let p = sim.config().riders()[0].physiology().unwrap();
    let expected = step_physiology(p, 1000.0, 5000.0, sim.config().fixed_step()).unwrap();
    let next = sim
        .step(&s, &f, sim.config().fixed_step())
        .unwrap()
        .next_state;
    assert_eq!(
        next.riders()[0].w_prime_balance_j(),
        Some(expected.balance_j)
    );
}
#[test]
fn physiology_snapshots_resume_exactly_and_configuration_stays_fixed() {
    let (sim, mut state, frame) = setup(20_000.0);
    let original = sim.config().clone();
    for _ in 0..300 {
        state = sim
            .step(&state, &frame, sim.config().fixed_step())
            .unwrap()
            .next_state;
    }
    let bytes = encode_snapshot(sim.config(), &state).unwrap();
    let (c, mut resumed) = decode_snapshot(&bytes).unwrap().into_parts();
    let sim2 = RaceSimulation::new(c).unwrap();
    assert_eq!(encode_snapshot(sim2.config(), &resumed).unwrap(), bytes);
    for _ in 0..300 {
        let a = sim.step(&state, &frame, sim.config().fixed_step()).unwrap();
        let b = sim2
            .step(&resumed, &frame, sim2.config().fixed_step())
            .unwrap();
        assert_eq!(a, b);
        state = a.next_state;
        resumed = b.next_state;
    }
    assert_eq!(sim.config(), &original);
}
#[test]
fn malformed_capability_or_reserve_is_rejected_with_a_path() {
    let (sim, s, _) = setup(1000.0);
    let bytes = encode_snapshot(sim.config(), &s).unwrap();
    for (bad, path) in [
        (
            bytes.replace("\"wPrimeBalanceJ\":1000.0", "\"wPrimeBalanceJ\":20001.0"),
            "state.riders[0].wPrimeBalanceJ",
        ),
        (
            bytes.replace("\"wPrimeBalanceJ\":1000.0", "\"wPrimeBalanceJ\":null"),
            "state.riders[0].wPrimeBalanceJ",
        ),
        (
            bytes.replace("\"criticalPowerW\":250.0", "\"criticalPowerW\":0.0"),
            "config.riders[0].physiology",
        ),
        (
            bytes.replace("\"wPrimeBalanceJ\":1000.0", "\"wPrimeBalanceJ\":-1.0"),
            "state.riders[0].wPrimeBalanceJ",
        ),
    ] {
        assert_ne!(bad, bytes);
        let e = decode_snapshot(&bad).unwrap_err();
        assert_eq!(e.path(), path);
    }
}
#[test]
fn pace_plans_are_reproducible_and_select_recovery_when_tired() {
    use race_engine::pack::PackPacePlan;
    let (sim, full, _) = setup(20_000.0);
    let (_, tired, _) = setup(0.0);
    let d = &sim.config().riders()[0];
    let p = PackPacePlan::seeded(42);
    assert_eq!(p, PackPacePlan::seeded(42));
    let mut saw_surge = false;
    for i in 0..20 {
        let t = Duration::from_secs(i * 30);
        let fresh = p.requested_power(d, &full.riders()[0], t).unwrap();
        let exhausted = p.requested_power(d, &tired.riders()[0], t).unwrap();
        assert!(exhausted < d.physiology().unwrap().critical_power_w());
        assert!(fresh >= exhausted);
        saw_surge |= fresh > 250.0;
    }
    assert!(
        saw_surge,
        "fixed seed covers a surge in the generated schedule"
    );
}
