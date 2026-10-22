//! Workload, geometry, replay and physical-continuity acceptance tests.
use race_engine::{mixed::*, observation::*, *};
use std::{sync::OnceLock, time::Duration};
use workout_import::WorkoutTimeline;

fn fixture() -> MixedSession {
    static INITIAL: OnceLock<MixedSession> = OnceLock::new();
    INITIAL
        .get_or_init(|| MixedSession::new(MixedOptions::default()).unwrap())
        .clone()
}
fn geometry(t: u64, positions: [f64; 6]) -> (RaceSimulationConfig, RaceState) {
    let c = fixture().config().clone();
    let riders = c
        .riders()
        .iter()
        .zip(positions)
        .map(|(d, x)| {
            RiderState::new(d.id().clone(), x, 10.0, 1, None)
                .unwrap()
                .with_w_prime_balance(20000.0)
                .unwrap()
        })
        .collect();
    let state =
        RaceState::new(Duration::from_secs(t), Pcg32::seeded(1, 1).state(), riders).unwrap();
    (c, state)
}
#[test]
fn exact_workout_boundaries_and_duration() {
    let t = WorkoutTimeline::new(session_director::mixed::workout()).unwrap();
    assert_eq!(t.total_duration(), Duration::from_secs(3600));
    for (s, w) in [
        (0, 135.0),
        (600, 204.0),
        (1500, 330.0),
        (1680, 204.0),
        (2280, 246.0),
        (2640, 360.0),
        (2700, 204.0),
        (3300, 165.0),
        (3600, 105.0),
    ] {
        assert!((t.power_at(Duration::from_secs(s), 300.0).unwrap().unwrap() - w).abs() < 1e-9);
    }
}
#[test]
fn geometry_and_explicit_intent_are_independent() {
    let (c, s) = geometry(0, [100.0, 96.0, 92.0, 50.0, 46.0, 42.0]);
    let mut o = RaceObserver::default();
    assert_eq!(
        o.observe(&c, &s, ChaseIntent::None).unwrap().topology(),
        Topology::Forming
    );
    let (_, s5) = geometry(5, [150.0, 146.0, 142.0, 100.0, 96.0, 92.0]);
    let v = o.observe(&c, &s5, ChaseIntent::None).unwrap();
    assert_eq!(v.topology(), Topology::Established);
    assert_eq!(v.pursuit(), Pursuit::Unchallenged);
    let (_, s10) = geometry(10, [200.0, 196.0, 192.0, 160.0, 156.0, 152.0]);
    let v = o.observe(&c, &s10, ChaseIntent::Pursue).unwrap();
    assert_eq!(v.gap_rate_ms(), Some(-2.0));
    assert_eq!(v.pursuit(), Pursuit::Closing);
    assert_eq!(o.observe(&c, &s10, ChaseIntent::Pursue).unwrap(), v);
    assert!(o.observe(&c, &s10, ChaseIntent::Manage).is_err());
    let (_, conflict) = geometry(10, [201.0, 196.0, 192.0, 160.0, 156.0, 152.0]);
    assert!(o.observe(&c, &conflict, ChaseIntent::Pursue).is_err());
    assert!(o.observe(&c, &s5, ChaseIntent::None).is_err());
    assert_eq!(o.observe(&c, &s10, ChaseIntent::Pursue).unwrap(), v);
    let (_, merge) = geometry(11, [210.0, 206.0, 202.0, 198.0, 194.0, 190.0]);
    assert_eq!(
        o.observe(&c, &merge, ChaseIntent::None).unwrap().topology(),
        Topology::Regrouping
    );
}
#[test]
fn validation_errors_leave_policy_and_state_unchanged() {
    assert!(MixedOptions::new(f64::NAN, 42, true).is_err());
    assert!(ObserverTuning::new(0.0, Duration::from_secs(1), Duration::from_secs(1), 0.1).is_err());
    let mut a = fixture();
    let mut b = fixture();
    let initial = a.state().clone();
    for (w, slot, wheel) in [
        (f64::NAN, 1, None),
        (200.0, 3, None),
        (200.0, 1, Some("unknown")),
        (200.0, 1, Some("human")),
    ] {
        assert!(a.step(w, slot, wheel).is_err());
        assert_eq!(a.state(), &initial);
    }
    assert_eq!(
        a.step(135.0, 1, None).unwrap().inputs,
        b.step(135.0, 1, None).unwrap().inputs
    );
    assert_eq!(a.state(), b.state());
}
#[test]
fn ordinary_inputs_replay_every_state_and_event_exactly() {
    let mut a = fixture();
    let sim = RaceSimulation::new(a.config().clone()).unwrap();
    let mut b = a.state().clone();
    for _ in 0..1200 {
        let step = a.step(a.target_w(), 1, None).unwrap();
        let replay = sim
            .step(&b, &step.inputs, sim.config().fixed_step())
            .unwrap();
        assert_eq!(replay.next_state, *a.state());
        assert_eq!(replay.events, step.events);
        b = replay.next_state;
    }
}
#[test]
fn full_hour_keeps_physics_and_steady_shelter_and_finite_reserve() {
    let mut s = fixture();
    let config = s.config().clone();
    let mut sheltered = [0u32; 8];
    let mut count = [0u32; 8];
    let mut previous = s.state().clone();
    while !s.is_complete() {
        let step = s.step(s.target_w(), 1, None).unwrap();
        let h = step.sample.riders.iter().find(|r| r.id == PLAYER).unwrap();
        count[step.sample.block] += 1;
        sheltered[step.sample.block] += u32::from(h.wheel.is_some());
        for (r, p) in s.state().riders().iter().zip(previous.riders()) {
            assert!(r.distance_m() >= p.distance_m());
            assert!((0.0..=20000.0).contains(&r.w_prime_balance_j().unwrap()));
        }
        if s.state().simulation_time() == Duration::from_secs(600) {
            let front = step
                .sample
                .riders
                .iter()
                .map(|r| r.distance_m)
                .max_by(f64::total_cmp)
                .unwrap();
            assert!(
                front - h.distance_m < 50.0,
                "warm-up must retain a nearby local field"
            );
        }
        if s.state().simulation_time() == Duration::from_secs(1680) {
            assert!((h.reserve_j - 14600.0).abs() < 1e-6);
        }
        previous = s.state().clone();
    }
    assert_eq!(s.config(), &config);
    assert_eq!(s.state().simulation_time(), Duration::from_secs(3600));
    for block in [1, 3, 6] {
        assert!(
            f64::from(sheltered[block]) / f64::from(count[block]) > 0.90,
            "block {block}: {}/{}",
            sheltered[block],
            count[block]
        );
    }
    assert!(s.step(200.0, 1, None).is_err());
}
#[test]
fn missed_chase_has_persistent_cost_without_pacer_rescue() {
    let mut reference = fixture();
    let mut missed = fixture();
    while reference.state().simulation_time() < Duration::from_secs(1740) {
        let block = reference.block();
        let a = reference.step(reference.target_w(), 1, None).unwrap();
        let b = missed
            .step(if block == 2 { 204.0 } else { missed.target_w() }, 1, None)
            .unwrap();
        let pacer = RiderId::new("pacer").unwrap();
        assert_eq!(a.inputs.input_for(&pacer), b.inputs.input_for(&pacer));
    }
    let id = RiderId::new(PLAYER).unwrap();
    assert!(
        reference.state().rider_state(&id).unwrap().distance_m()
            - missed.state().rider_state(&id).unwrap().distance_m()
            > 50.0
    );
    assert_eq!(missed.last_sample().unwrap().scene, Scene::SoloFallback);
}
#[test]
fn power_is_physiology_limited_and_position_changes_cost() {
    let mut high = fixture();
    assert!(high.step(10000.0, 1, None).unwrap().sample.delivered_w <= 1200.0);
    let mut a = fixture();
    let mut b = fixture();
    for _ in 0..2400 {
        a.step(a.target_w(), 1, None).unwrap();
        b.step(b.target_w(), 2, None).unwrap();
    }
    let id = RiderId::new(PLAYER).unwrap();
    assert!(
        a.state().rider_state(&id).unwrap().distance_m()
            - b.state().rider_state(&id).unwrap().distance_m()
            > 10.0
    );
}
