//! The event vocabulary, from its definition: each test builds the situation an event describes
//! and checks the kernel reports exactly that, and nothing that did not happen.

use std::time::Duration;

use race_engine::{
    BasicDraftingConfig, BeyondEndBehavior, Pcg32, RaceEvent, RaceEventKind, RaceInputFrame,
    RaceSimulation, RaceSimulationConfig, RaceSimulationConfigSpec, RaceState, RiderControlInput,
    RiderDefinition, RiderId, RiderState, SimulationTerrain, SimulationValidationCode as C,
    TerrainInterpolation, TerrainSample,
};
use race_physics::{LongitudinalDynamicsConfig, RiderPhysicalParams};

const STEP: Duration = Duration::from_millis(50);

fn id(s: &str) -> RiderId {
    RiderId::new(s).unwrap()
}

fn config(
    riders: &[&str],
    drafting: Option<BasicDraftingConfig>,
    length_m: Option<f64>,
) -> RaceSimulationConfig {
    RaceSimulationConfig::new(RaceSimulationConfigSpec {
        fixed_step: STEP,
        riders: riders
            .iter()
            .map(|r| RiderDefinition::new(id(r), RiderPhysicalParams::new(75.0)).unwrap())
            .collect(),
        terrain: SimulationTerrain::new(
            vec![TerrainSample::new(0.0, 0.0).unwrap()],
            TerrainInterpolation::Step,
            BeyondEndBehavior::Flat,
            length_m,
        )
        .unwrap(),
        dynamics: LongitudinalDynamicsConfig::default(),
        drafting,
        lateral_slot_count: 3,
        air_density_kg_m3: 1.225,
        headwind_ms: 0.0,
    })
    .unwrap()
}

fn state(riders: Vec<RiderState>) -> RaceState {
    RaceState::new(Duration::ZERO, Pcg32::seeded(7, 1).state(), riders).unwrap()
}

fn rider(
    name: &str,
    distance_m: f64,
    speed_ms: f64,
    slot: i64,
    target: Option<&str>,
) -> RiderState {
    RiderState::new(id(name), distance_m, speed_ms, slot, target.map(id)).unwrap()
}

fn frame(inputs: &[(&str, f64, Option<i64>)]) -> RaceInputFrame {
    RaceInputFrame::new(
        inputs
            .iter()
            .map(|&(r, watts, slot)| RiderControlInput::new(id(r), watts, 0.0, slot, None).unwrap())
            .collect(),
    )
    .unwrap()
}

fn kinds(events: &[RaceEvent]) -> Vec<RaceEventKind> {
    events.iter().map(|e| e.kind().clone()).collect()
}

#[test]
fn a_wheel_is_lost_when_there_is_no_drafting_model_to_keep_it() {
    let sim = RaceSimulation::new(config(&["a", "b"], None, None)).unwrap();
    let before = state(vec![
        rider("a", 3.0, 10.0, 0, None),
        rider("b", 0.0, 10.0, 0, Some("a")),
    ]);
    let result = sim
        .step(
            &before,
            &frame(&[("a", 200.0, None), ("b", 200.0, None)]),
            STEP,
        )
        .unwrap();
    assert_eq!(
        kinds(&result.events),
        vec![RaceEventKind::WheelLost {
            rider: id("b"),
            target: id("a")
        }]
    );
    assert_eq!(result.events[0].simulation_time(), STEP);
}

#[test]
fn taking_a_wheel_and_moving_slot_are_reported_with_their_values() {
    let drafting = BasicDraftingConfig::new(5.0, vec![0.6, 0.85]).unwrap();
    let sim = RaceSimulation::new(config(&["a", "b"], Some(drafting), None)).unwrap();
    // b sits 3 m behind a, one slot over, without a wheel; it asks for a's slot.
    let before = state(vec![
        rider("a", 3.0, 10.0, 0, None),
        rider("b", 0.0, 10.0, 1, None),
    ]);
    let result = sim
        .step(
            &before,
            &frame(&[("a", 200.0, None), ("b", 200.0, Some(0))]),
            STEP,
        )
        .unwrap();
    assert_eq!(
        kinds(&result.events),
        vec![
            RaceEventKind::SlotChanged {
                rider: id("b"),
                from_slot: 1,
                to_slot: 0
            },
            RaceEventKind::WheelTaken {
                rider: id("b"),
                target: id("a")
            },
        ]
    );
}

#[test]
fn an_overtake_is_reported_once_on_the_step_the_order_flips() {
    let sim = RaceSimulation::new(config(&["a", "b"], None, None)).unwrap();
    let mut s = state(vec![
        rider("a", 1.0, 10.0, 0, None),
        rider("b", 0.0, 10.0, 1, None),
    ]);
    let inputs = frame(&[("a", 0.0, None), ("b", 600.0, None)]);
    let mut overtakes = Vec::new();
    let mut flipped_at = None;
    for _ in 0..200 {
        let result = sim.step(&s, &inputs, STEP).unwrap();
        let ahead = |st: &RaceState, r: &str| st.rider_state(&id(r)).unwrap().distance_m();
        if flipped_at.is_none() && ahead(&result.next_state, "b") > ahead(&result.next_state, "a") {
            flipped_at = Some(result.next_state.simulation_time());
        }
        overtakes.extend(
            result
                .events
                .into_iter()
                .filter(|e| matches!(e.kind(), RaceEventKind::Overtook { .. })),
        );
        s = result.next_state;
    }
    assert_eq!(overtakes.len(), 1);
    assert_eq!(
        overtakes[0].kind(),
        &RaceEventKind::Overtook {
            rider: id("b"),
            passed: id("a")
        }
    );
    assert_eq!(Some(overtakes[0].simulation_time()), flipped_at);
}

#[test]
fn same_step_finishers_are_placed_by_when_they_crossed_not_by_id() {
    // Both cross the 100 m line in the first step: b needs 0.1 m, a needs 0.3 m.
    let sim = RaceSimulation::new(config(&["a", "b"], None, Some(100.0))).unwrap();
    let before = state(vec![
        rider("a", 99.7, 10.0, 0, None),
        rider("b", 99.9, 10.0, 1, None),
    ]);
    let result = sim
        .step(
            &before,
            &frame(&[("a", 200.0, None), ("b", 200.0, None)]),
            STEP,
        )
        .unwrap();
    assert_eq!(
        kinds(&result.events),
        vec![
            RaceEventKind::Finished {
                rider: id("b"),
                place: 1
            },
            RaceEventKind::Finished {
                rider: id("a"),
                place: 2
            },
        ]
    );
}

#[test]
fn a_rider_already_past_the_line_is_never_finished_again_and_holds_its_place() {
    let sim = RaceSimulation::new(config(&["a", "b"], None, Some(100.0))).unwrap();
    let before = state(vec![
        rider("a", 120.0, 10.0, 0, None),
        rider("b", 99.9, 10.0, 1, None),
    ]);
    let result = sim
        .step(
            &before,
            &frame(&[("a", 200.0, None), ("b", 200.0, None)]),
            STEP,
        )
        .unwrap();
    assert_eq!(
        kinds(&result.events),
        vec![RaceEventKind::Finished {
            rider: id("b"),
            place: 2
        }]
    );
}

#[test]
fn a_course_without_a_length_has_no_finish() {
    let sim = RaceSimulation::new(config(&["a"], None, None)).unwrap();
    let mut s = state(vec![rider("a", 0.0, 10.0, 0, None)]);
    for _ in 0..400 {
        let result = sim.step(&s, &frame(&[("a", 300.0, None)]), STEP).unwrap();
        assert!(result.events.is_empty());
        s = result.next_state;
    }
}

#[test]
fn malformed_events_are_rejected_with_a_code() {
    let err = |kind| RaceEvent::new(STEP, kind).unwrap_err().validation_code();
    assert_eq!(
        err(RaceEventKind::Overtook {
            rider: id("a"),
            passed: id("a")
        }),
        Some(C::DuplicateRiderId)
    );
    assert_eq!(
        err(RaceEventKind::SlotChanged {
            rider: id("a"),
            from_slot: 1,
            to_slot: 1
        }),
        Some(C::OutOfRange)
    );
    assert_eq!(
        err(RaceEventKind::Finished {
            rider: id("a"),
            place: 0
        }),
        Some(C::NotPositive)
    );
    let ok = RaceEvent::new(
        STEP,
        RaceEventKind::WheelTaken {
            rider: id("b"),
            target: id("a"),
        },
    )
    .unwrap();
    assert_eq!(ok.involved_rider_ids(), vec![&id("a"), &id("b")]);
}
