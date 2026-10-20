//! Regression tests for capability and geometry invariants.
use race_engine::simulation::positioning::{
    compute_drag_multipliers, resolve_lateral_slots, select_draft_targets,
};
use race_engine::*;
use race_physics::{LongitudinalDynamicsConfig, RiderPhysicalParams};
use std::time::Duration;
fn inputs(gap: f64, offset: i64) -> (RaceSimulationConfig, RaceState, RaceInputFrame) {
    let id = |s| RiderId::new(s).unwrap();
    let mut spec = RaceSimulationConfigSpec::new(
        Duration::from_millis(50),
        ["a", "b"]
            .map(|s| RiderDefinition::new(id(s), RiderPhysicalParams::new(75.0)).unwrap())
            .to_vec(),
        SimulationTerrain::flat(0.0).unwrap(),
        LongitudinalDynamicsConfig::default(),
        Some(BasicDraftingConfig::with_min_gap(6.0, vec![0.6, 0.85], 2.5).unwrap()),
    );
    spec.lateral_slot_count = 3;
    let c = RaceSimulationConfig::new(spec).unwrap();
    let s = RaceState::new(
        Duration::ZERO,
        Pcg32::seeded(1, 1).state(),
        vec![
            RiderState::new(id("a"), 0.0, 10.0, 0, None).unwrap(),
            RiderState::new(id("b"), gap, 10.0, offset, None).unwrap(),
        ],
    )
    .unwrap();
    let f = RaceInputFrame::new(
        ["a", "b"]
            .map(|s| RiderControlInput::power(id(s), 250.0).unwrap())
            .to_vec(),
    )
    .unwrap();
    (c, s, f)
}
fn drag(gap: f64, offset: i64) -> f64 {
    let (c, s, f) = inputs(gap, offset);
    let slots = resolve_lateral_slots(&s, &f);
    let targets = select_draft_targets(&c, &s, &slots, &f);
    compute_drag_multipliers(&c, &s, &slots, &targets)[0]
}
#[test]
fn overlapping_and_abreast_riders_never_shelter_each_other() {
    for offset in [0, 1, 2] {
        for gap in [0.0, 0.01, 1.8, 2.49, 2.5] {
            assert_eq!(drag(gap, offset), 1.0);
        }
    }
    assert!(
        drag(2.5001, 1) < 1.0,
        "diagonal shelter requires longitudinal separation"
    );
    assert_eq!(drag(3.0, 2), 1.0);
}
#[test]
fn shelter_decays_from_minimum_to_maximum_gap() {
    // halfway between 2.5 and 6: 0.6 + (1-.6)*.5 = .8
    assert!((drag(4.25, 0) - 0.8).abs() < 1e-12);
    assert!(drag(3.0, 0) < drag(4.0, 0));
    assert!(drag(4.0, 0) < drag(5.0, 0));
    assert_eq!(drag(6.0, 0), 1.0);
    assert_eq!(drag(7.0, 0), 1.0);
}
#[test]
fn minimum_gap_is_validated_and_persisted() {
    for min in [-1.0, 6.0, 7.0, f64::NAN] {
        assert!(BasicDraftingConfig::with_min_gap(6.0, vec![0.6], min).is_err());
    }
    let (c, s, _) = inputs(3.0, 0);
    let text = encode_snapshot(&c, &s).unwrap();
    let snap = decode_snapshot(&text).unwrap();
    assert_eq!(snap.config().drafting().unwrap().min_gap_m(), 2.5);
}
