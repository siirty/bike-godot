//! The headless corpus: every seeded race runs to the finish without breaking an invariant, and
//! its event stream agrees with independent reconstructions from the states it describes.

use std::collections::BTreeMap;

use race_engine::harness::{run_race, CorpusScenario, RaceRun};
use race_engine::{
    decode_snapshot, encode_snapshot, RaceEventKind, RaceSimulation, RaceState, RiderId,
};

const SEEDS: std::ops::Range<u64> = 0..12;

fn run(seed: u64) -> (CorpusScenario, RaceRun) {
    let scenario = CorpusScenario::generate(seed).unwrap();
    let run = scenario
        .run()
        .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
    (scenario, run)
}

#[test]
fn every_seed_finishes_with_each_place_awarded_exactly_once() {
    for seed in SEEDS {
        let (scenario, run) = run(seed);
        let n = scenario.config.riders().len();
        let finishers = run.finishers();
        let places: Vec<u32> = finishers.iter().map(|f| f.1).collect();
        assert_eq!(places, (1..=n as u32).collect::<Vec<_>>(), "seed {seed}");
        let mut riders: Vec<&RiderId> = finishers.iter().map(|f| &f.0).collect();
        riders.sort();
        riders.dedup();
        assert_eq!(riders.len(), n, "seed {seed}: a rider finished twice");
        // A later place never has an earlier time.
        assert!(
            finishers.windows(2).all(|w| w[0].2 <= w[1].2),
            "seed {seed}"
        );
        // Plausible road-race speeds for the winner: an explicit sanity band, not a tuned value.
        let length = scenario.config.terrain().end_distance_m();
        let kmh = length / finishers[0].2.as_secs_f64() * 3.6;
        assert!(
            (15.0..60.0).contains(&kmh),
            "seed {seed}: winner at {kmh:.1} km/h"
        );
    }
}

#[test]
fn a_seed_reproduces_the_same_race_byte_for_byte() {
    let (scenario, a) = run(3);
    let (_, b) = run(3);
    assert_eq!(a.events, b.events);
    assert_eq!(
        encode_snapshot(&scenario.config, &a.final_state).unwrap(),
        encode_snapshot(&scenario.config, &b.final_state).unwrap()
    );
}

#[test]
fn a_race_resumed_from_a_snapshot_reports_exactly_the_remaining_events() {
    let (scenario, uninterrupted) = run(5);
    let split = uninterrupted.steps / 2;
    let sim = RaceSimulation::new(scenario.config.clone()).unwrap();
    let first = run_race(&sim, scenario.initial_state.clone(), split, |s| {
        scenario.inputs_at(s)
    })
    .unwrap();
    let bytes = encode_snapshot(&scenario.config, &first.final_state).unwrap();
    let (config, state) = decode_snapshot(&bytes).unwrap().into_parts();
    let resumed_sim = RaceSimulation::new(config).unwrap();
    let second = run_race(&resumed_sim, state, scenario.max_steps - split, |s| {
        scenario.inputs_at(s)
    })
    .unwrap();

    let mut events = first.events;
    events.extend(second.events);
    assert_eq!(events, uninterrupted.events);
    assert_eq!(
        encode_snapshot(&scenario.config, &second.final_state).unwrap(),
        encode_snapshot(&scenario.config, &uninterrupted.final_state).unwrap()
    );
}

fn ahead(state: &RaceState, a: &RiderId, b: &RiderId) -> i32 {
    let d = |r: &RiderId| state.rider_state(r).unwrap().distance_m();
    i32::from(d(a) > d(b))
}

#[test]
fn overtakes_account_for_every_change_in_running_order() {
    // For each ordered pair, passes(a over b) − passes(b over a) must equal how a's position
    // relative to b changed between the start and the end. Derived from the states alone.
    for seed in SEEDS {
        let (scenario, run) = run(seed);
        let mut net: BTreeMap<(RiderId, RiderId), i32> = BTreeMap::new();
        for e in &run.events {
            if let RaceEventKind::Overtook { rider, passed } = e.kind() {
                *net.entry((rider.clone(), passed.clone())).or_default() += 1;
                *net.entry((passed.clone(), rider.clone())).or_default() -= 1;
            }
        }
        let ids: Vec<RiderId> = scenario
            .config
            .riders()
            .iter()
            .map(|r| r.id().clone())
            .collect();
        for a in &ids {
            for b in ids.iter().filter(|b| *b != a) {
                let expected = ahead(&run.final_state, a, b) - ahead(&scenario.initial_state, a, b);
                let got = net.get(&(a.clone(), b.clone())).copied().unwrap_or(0);
                assert_eq!(got, expected, "seed {seed}: {a} vs {b}");
            }
        }
    }
}

#[test]
fn replaying_wheel_and_slot_events_reconstructs_the_final_state() {
    for seed in SEEDS {
        let (scenario, run) = run(seed);
        let mut wheel: BTreeMap<RiderId, Option<RiderId>> = BTreeMap::new();
        let mut slot: BTreeMap<RiderId, usize> = BTreeMap::new();
        for r in scenario.initial_state.riders() {
            wheel.insert(r.id().clone(), r.draft_target_id().cloned());
            slot.insert(r.id().clone(), r.lateral_slot());
        }
        for e in &run.events {
            match e.kind() {
                RaceEventKind::WheelLost { rider, target } => {
                    assert_eq!(wheel[rider].as_ref(), Some(target), "seed {seed}");
                    wheel.insert(rider.clone(), None);
                }
                RaceEventKind::WheelTaken { rider, target } => {
                    assert_eq!(
                        wheel[rider], None,
                        "seed {seed}: took a wheel while holding one"
                    );
                    wheel.insert(rider.clone(), Some(target.clone()));
                }
                RaceEventKind::SlotChanged {
                    rider,
                    from_slot,
                    to_slot,
                } => {
                    assert_eq!(slot[rider], *from_slot, "seed {seed}");
                    slot.insert(rider.clone(), *to_slot);
                }
                _ => {}
            }
        }
        for r in run.final_state.riders() {
            assert_eq!(
                wheel[r.id()].as_ref(),
                r.draft_target_id(),
                "seed {seed}: {}",
                r.id()
            );
            assert_eq!(slot[r.id()], r.lateral_slot(), "seed {seed}: {}", r.id());
        }
    }
}
