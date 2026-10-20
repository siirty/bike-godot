//! Kernel invariants that hold for any input, derived from the accepted contracts rather than
//! from recorded output.

use std::time::Duration;

use race_engine::testing::DeterministicComparison;
use race_engine::{
    decode_snapshot, encode_snapshot, BasicDraftingConfig, Pcg32, RaceInputFrame, RaceSimulation,
    RaceSimulationConfig, RaceSimulationConfigSpec, RaceState, RiderControlInput, RiderDefinition,
    RiderId, RiderState, SimulationTerrain, SimulationValidationCode as C, SnapshotDecodeCode,
};
use race_physics::{LongitudinalDynamicsConfig, RiderPhysicalParams};

const STEP: Duration = Duration::from_millis(50);

fn id(s: &str) -> RiderId {
    RiderId::new(s).unwrap()
}

fn config(drafting: Option<BasicDraftingConfig>) -> RaceSimulationConfig {
    RaceSimulationConfig::new(RaceSimulationConfigSpec {
        fixed_step: STEP,
        riders: ["a", "b", "c"]
            .map(|r| RiderDefinition::new(id(r), RiderPhysicalParams::new(75.0)).unwrap())
            .to_vec(),
        terrain: SimulationTerrain::flat(0.0).unwrap(),
        dynamics: LongitudinalDynamicsConfig::default(),
        drafting,
        lateral_slot_count: 3,
        air_density_kg_m3: 1.225,
        headwind_ms: 0.0,
    })
    .unwrap()
}

fn drafting() -> BasicDraftingConfig {
    BasicDraftingConfig::new(5.0, vec![0.6, 0.85]).unwrap()
}

/// `a` leads by 3 m; `b` and `c` sit behind it in the same slot.
fn start_state() -> RaceState {
    RaceState::new(
        Duration::ZERO,
        Pcg32::seeded(1, 2).state(),
        vec![
            RiderState::new(id("a"), 3.0, 10.0, 0, None).unwrap(),
            RiderState::new(id("b"), 0.0, 10.0, 0, Some(id("a"))).unwrap(),
            RiderState::new(id("c"), 0.0, 10.0, 2, None).unwrap(),
        ],
    )
    .unwrap()
}

fn same_power(watts: f64) -> RaceInputFrame {
    RaceInputFrame::new(
        ["a", "b", "c"]
            .map(|r| RiderControlInput::power(id(r), watts).unwrap())
            .to_vec(),
    )
    .unwrap()
}

fn run(
    sim: &RaceSimulation,
    mut state: RaceState,
    steps: usize,
    frame: &RaceInputFrame,
) -> RaceState {
    for _ in 0..steps {
        state = sim.step(&state, frame, STEP).unwrap().next_state;
    }
    state
}

#[test]
fn the_same_inputs_give_bit_identical_states() {
    let sim = RaceSimulation::new(config(Some(drafting()))).unwrap();
    let a = run(&sim, start_state(), 400, &same_power(240.0));
    let b = run(&sim, start_state(), 400, &same_power(240.0));
    assert_eq!(a, b);
    assert!(DeterministicComparison::exact()
        .compare_states(&a, &b, "state")
        .is_empty());
}

#[test]
fn resuming_from_a_snapshot_matches_an_uninterrupted_race_exactly() {
    let config = config(Some(drafting()));
    let sim = RaceSimulation::new(config.clone()).unwrap();
    let frame = same_power(260.0);
    let uninterrupted = run(&sim, start_state(), 600, &frame);

    let halfway = run(&sim, start_state(), 300, &frame);
    let bytes = encode_snapshot(&config, &halfway).unwrap();
    let (config2, state2) = decode_snapshot(&bytes).unwrap().into_parts();
    assert_eq!(config2, config);
    let resumed = run(&RaceSimulation::new(config2).unwrap(), state2, 300, &frame);

    let diffs = DeterministicComparison::exact().compare_states(&resumed, &uninterrupted, "state");
    assert!(diffs.is_empty(), "{}", diffs.join("\n"));
    assert_eq!(
        encode_snapshot(&config, &resumed).unwrap(),
        encode_snapshot(&config, &uninterrupted).unwrap()
    );
}

#[test]
fn a_step_advances_time_by_the_fixed_step_and_draws_no_randomness() {
    let sim = RaceSimulation::new(config(None)).unwrap();
    let before = start_state();
    let result = sim.step(&before, &same_power(200.0), STEP).unwrap();
    assert_eq!(result.next_state.simulation_time(), STEP);
    assert_eq!(result.next_state.random_state(), before.random_state());
}

#[test]
fn a_step_never_rewrites_the_configuration() {
    let config = config(Some(drafting()));
    let sim = RaceSimulation::new(config.clone()).unwrap();
    run(&sim, start_state(), 100, &same_power(400.0));
    assert_eq!(sim.config(), &config);
}

#[test]
fn shelter_only_helps_through_aero_drag() {
    // Same power, same mass: the sheltered rider ends up faster than with drafting off, and
    // nobody is faster than their unsheltered solo speed would allow at zero drag.
    let with = run(
        &RaceSimulation::new(config(Some(drafting()))).unwrap(),
        start_state(),
        20,
        &same_power(250.0),
    );
    let without = run(
        &RaceSimulation::new(config(None)).unwrap(),
        start_state(),
        20,
        &same_power(250.0),
    );
    let speed = |s: &RaceState, r: &str| s.rider_state(&id(r)).unwrap().speed_ms();
    assert!(speed(&with, "b") > speed(&without, "b"));
    // The leader and the rider two slots across get no shelter: identical to the undrafted run.
    assert_eq!(speed(&with, "a").to_bits(), speed(&without, "a").to_bits());
    assert_eq!(speed(&with, "c").to_bits(), speed(&without, "c").to_bits());
}

#[test]
fn variable_or_zero_timesteps_are_rejected_without_effect() {
    let sim = RaceSimulation::new(config(None)).unwrap();
    let state = start_state();
    let e = sim
        .step(&state, &same_power(200.0), Duration::from_millis(49))
        .unwrap_err();
    assert_eq!(
        (e.validation_code(), e.path()),
        (Some(C::TimestepMismatch), "dt")
    );
    let e = sim
        .step(&state, &same_power(200.0), Duration::ZERO)
        .unwrap_err();
    assert_eq!(
        (e.validation_code(), e.path()),
        (Some(C::NotPositive), "dt")
    );
}

#[test]
fn every_configured_rider_needs_an_input_and_only_they_may_have_one() {
    let sim = RaceSimulation::new(config(None)).unwrap();
    let partial = RaceInputFrame::new(vec![
        RiderControlInput::power(id("a"), 1.0).unwrap(),
        RiderControlInput::power(id("b"), 1.0).unwrap(),
    ])
    .unwrap();
    let e = sim.step(&start_state(), &partial, STEP).unwrap_err();
    assert_eq!(e.validation_code(), Some(C::MissingRiderId));

    let extra = RaceInputFrame::new(
        ["a", "b", "c", "z"]
            .map(|r| RiderControlInput::power(id(r), 1.0).unwrap())
            .to_vec(),
    )
    .unwrap();
    let e = sim.step(&start_state(), &extra, STEP).unwrap_err();
    assert_eq!(
        (e.validation_code(), e.path()),
        (Some(C::UnknownRiderId), "inputs.inputs[3]")
    );
}

#[test]
fn rider_ids_order_by_utf16_code_units_not_bytes() {
    // U+FF21 (fullwidth A) is one UTF-16 unit 0xFF21; U+1F600 is the surrogate pair D83D DE00.
    // Bytewise UTF-8 puts the emoji last; UTF-16 (the prototype's String order) puts it first.
    let mut ids = [id("\u{FF21}"), id("\u{1F600}")];
    ids.sort();
    assert_eq!(ids[0].as_str(), "\u{1F600}");
}

#[test]
fn a_decoded_non_positive_force_cap_is_a_typed_validation_error() {
    let config = config(None);
    let bytes = encode_snapshot(&config, &start_state()).unwrap();
    let broken = bytes.replace("\"maxPropulsiveForceN\":500.0", "\"maxPropulsiveForceN\":0");
    assert_ne!(broken, bytes);
    let e = decode_snapshot(&broken).unwrap_err();
    assert_eq!(e.validation_code(), Some(C::NotPositive));
    assert_eq!(e.path(), "config.dynamics.maxPropulsiveForceN");
}

#[test]
fn a_half_present_drafting_block_is_malformed() {
    let bytes = encode_snapshot(&config(None), &start_state()).unwrap();
    let broken = bytes.replace("\"maxGapM\":null", "\"maxGapM\":4.0");
    let e = decode_snapshot(&broken).unwrap_err();
    assert_eq!(
        (e.decode_code(), e.path()),
        (Some(SnapshotDecodeCode::MalformedJson), "config.drafting")
    );
}

#[test]
fn integer_json_numbers_decode_as_doubles_but_not_the_reverse() {
    let bytes = encode_snapshot(&config(None), &start_state()).unwrap();
    // 1.225 written as an integer-valued double elsewhere is fine; an int field as 3.0 is not.
    assert!(decode_snapshot(&bytes.replace("\"headwindMs\":0.0", "\"headwindMs\":0")).is_ok());
    let e = decode_snapshot(&bytes.replace("\"lateralSlotCount\":3", "\"lateralSlotCount\":3.0"))
        .unwrap_err();
    assert_eq!(
        (e.decode_code(), e.path()),
        (
            Some(SnapshotDecodeCode::WrongType),
            "config.lateralSlotCount"
        )
    );
}

#[test]
fn comparison_tolerates_rounding_but_not_discrete_differences() {
    let cmp = DeterministicComparison::default();
    assert!(cmp.doubles_match(1000.0, 1000.0 + 1e-7));
    assert!(!cmp.doubles_match(1000.0, 1000.001));
    assert!(!cmp.doubles_match(f64::NAN, f64::NAN));
    assert!(cmp.doubles_match(f64::INFINITY, f64::INFINITY));

    let a = start_state();
    let nudged = RaceState::new(
        a.simulation_time(),
        a.random_state(),
        a.riders()
            .iter()
            .map(|r| {
                RiderState::new(
                    r.id().clone(),
                    r.distance_m() + 1e-12,
                    r.speed_ms(),
                    r.lateral_slot() as i64,
                    r.draft_target_id().cloned(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    assert!(cmp.compare_states(&a, &nudged, "s").is_empty());
    // Exact mode reports every nudged distance.
    assert_eq!(
        DeterministicComparison::exact()
            .compare_states(&a, &nudged, "s")
            .len(),
        3
    );

    // A different slot is a different race, whatever the tolerance.
    let moved = RaceState::new(
        a.simulation_time(),
        a.random_state(),
        a.riders()
            .iter()
            .map(|r| {
                let slot = if r.id().as_str() == "c" {
                    1
                } else {
                    r.lateral_slot() as i64
                };
                RiderState::new(
                    r.id().clone(),
                    r.distance_m(),
                    r.speed_ms(),
                    slot,
                    r.draft_target_id().cloned(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let loose = DeterministicComparison {
        absolute_tolerance: 1e6,
        relative_tolerance: 1.0,
    };
    assert_eq!(
        loose.compare_states(&a, &moved, "s"),
        vec!["s.riders[2].lateralSlot: 2 != 1".to_owned()]
    );
}
