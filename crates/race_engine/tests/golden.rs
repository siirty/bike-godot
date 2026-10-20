//! Parity against vectors recorded from the Dart prototype (`fixtures/golden/race_engine.json`).
//! Doubles are compared bit for bit and snapshots byte for byte.

use std::collections::BTreeMap;
use std::path::PathBuf;

use race_engine::{
    decode_snapshot, encode_snapshot, hex64, BeyondEndBehavior, Pcg32, RaceInputFrame,
    RaceSimulation, RaceSimulationError, RiderControlInput, RiderId, SimulationTerrain,
    TerrainInterpolation, TerrainSample,
};
use serde_json::Value;

fn golden() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden/race_engine.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("read vectors")).expect("parse")
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

#[track_caller]
fn same(actual: f64, expected: &Value, context: &str) {
    let expected = f(expected);
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{context}: rust {actual:e} vs dart {expected:e}"
    );
}

/// Dart seeds are signed 64-bit; the generator works on their two's-complement bits.
fn seed(v: &Value) -> u64 {
    v.as_str().unwrap().parse::<i64>().unwrap() as u64
}

#[test]
fn pcg32_sequences_match() {
    let g = golden();
    for c in g["pcg32"].as_array().unwrap() {
        let ctx = format!("seed {} stream {}", c["seed"], c["stream"]);
        let mut rng = Pcg32::seeded(seed(&c["seed"]), seed(&c["stream"]));
        assert_eq!(hex64(rng.state().state()), c["startState"], "{ctx}");
        assert_eq!(hex64(rng.state().stream()), c["startStream"], "{ctx}");
        for e in c["u32"].as_array().unwrap() {
            assert_eq!(u64::from(rng.next_u32()), e.as_u64().unwrap(), "{ctx}");
        }
        for e in c["doubles"].as_array().unwrap() {
            same(rng.next_f64(), e, &ctx);
        }
        let bounds = [1u64, 2, 3, 7, 100, 1_000_003, 1 << 32];
        for (bound, e) in bounds.iter().zip(c["bounded"].as_array().unwrap()) {
            assert_eq!(
                rng.next_int_below(*bound).unwrap(),
                e.as_u64().unwrap(),
                "{ctx}"
            );
        }
        assert_eq!(hex64(rng.state().state()), c["endState"], "{ctx}");
    }
}

#[test]
fn terrain_lookups_match() {
    let g = golden();
    let cases = g["terrain"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for c in cases {
        let ctx = c.to_string();
        let samples = [(0.0, 0.01), (400.0, 0.065), (1150.5, -0.04), (2000.0, 0.02)]
            .map(|(d, gr)| TerrainSample::new(d, gr).unwrap())
            .to_vec();
        let terrain = SimulationTerrain::new(
            samples,
            TerrainInterpolation::by_name(c["interpolation"].as_str().unwrap()).unwrap(),
            BeyondEndBehavior::by_name(c["beyondEnd"].as_str().unwrap()).unwrap(),
            c["lengthM"].as_f64(),
        )
        .unwrap();
        for probe in c["probes"].as_array().unwrap() {
            same(terrain.grade_at(f(&probe[0])), &probe[1], &ctx);
        }
    }
}

/// `[fromStep, power, braking, slot|null, target|null]`; the latest entry at or before a step
/// applies.
fn inputs_at(script: &BTreeMap<String, Vec<Value>>, step: u64) -> RaceInputFrame {
    let inputs = script
        .iter()
        .map(|(id, entries)| {
            let e = entries
                .iter()
                .rev()
                .find(|e| e[0].as_u64().unwrap() <= step)
                .expect("an entry from step 0");
            RiderControlInput::new(
                RiderId::new(id.as_str()).unwrap(),
                f(&e[1]),
                f(&e[2]),
                e[3].as_i64(),
                e[4].as_str().map(|t| RiderId::new(t).unwrap()),
            )
            .unwrap()
        })
        .collect();
    RaceInputFrame::new(inputs).unwrap()
}

#[test]
fn historical_reference_races_replay_with_explicit_v3_test_fixtures() {
    let g = golden();
    for race in g["races"].as_array().unwrap() {
        let name = race["name"].as_str().unwrap();
        let initial = race["initialSnapshot"].as_str().unwrap();
        let (config, mut state) = decode_snapshot(&reference_fixture(initial))
            .unwrap_or_else(|e| panic!("{name}: {e}"))
            .into_parts();
        assert_eq!(
            reference_projection(&encode_snapshot(&config, &state).unwrap()),
            initial,
            "{name}: re-encode"
        );

        let script: BTreeMap<String, Vec<Value>> = race["inputScript"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_array().unwrap().clone()))
            .collect();
        let checkpoints: BTreeMap<u64, &Value> = race["checkpoints"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c["step"].as_u64().unwrap(), &c["riders"]))
            .collect();
        let simulation = RaceSimulation::new(config.clone()).unwrap();
        let steps = race["steps"].as_u64().unwrap();
        let mut seen = 0;
        for step in 0..steps {
            let frame = inputs_at(&script, step);
            state = simulation
                .step(&state, &frame, config.fixed_step())
                .unwrap_or_else(|e| panic!("{name} step {step}: {e}"))
                .next_state;
            if let Some(expected) = checkpoints.get(&(step + 1)) {
                seen += 1;
                let expected = expected.as_array().unwrap();
                assert_eq!(state.riders().len(), expected.len());
                for (actual, e) in state.riders().iter().zip(expected) {
                    let ctx = format!("{name} after step {}: {}", step + 1, e["id"]);
                    assert_eq!(actual.id().as_str(), e["id"], "{ctx}");
                    same(actual.distance_m(), &e["distanceM"], &ctx);
                    same(actual.speed_ms(), &e["speedMs"], &ctx);
                    assert_eq!(
                        actual.lateral_slot() as u64,
                        e["lateralSlot"].as_u64().unwrap(),
                        "{ctx}"
                    );
                    assert_eq!(
                        actual.draft_target_id().map(RiderId::as_str),
                        e["draftTargetId"].as_str(),
                        "{ctx}"
                    );
                }
            }
        }
        assert_eq!(seen, checkpoints.len(), "{name}: every checkpoint visited");
        assert_eq!(
            reference_projection(&encode_snapshot(&config, &state).unwrap()),
            race["finalSnapshot"].as_str().unwrap(),
            "{name}: final snapshot"
        );
    }
}

fn error_label(e: &RaceSimulationError) -> String {
    match (e.validation_code(), e.decode_code()) {
        (Some(code), _) => format!("validation:{}", code.name()),
        (_, Some(code)) => format!("decode:{}", code.name()),
        _ => unreachable!(),
    }
}

#[test]
fn decode_errors_match_code_path_and_message() {
    let g = golden();
    for c in g["decodeErrors"].as_array().unwrap() {
        let label = c["label"].as_str().unwrap();
        let err =
            decode_snapshot(&reference_fixture(c["source"].as_str().unwrap())).expect_err(label);
        assert_eq!(error_label(&err), c["error"], "{label}");
        assert_eq!(err.path(), c["path"], "{label}");
        if label == "v1" {
            assert!(err.message().contains("version 3"));
        } else if label == "not json" {
            // Parser wording differs between serde_json and dart:convert; the prefix is ours.
            assert!(
                err.message().starts_with("not valid JSON: "),
                "{label}: {}",
                err.message()
            );
        } else {
            assert_eq!(err.message(), c["message"], "{label}");
        }
    }
}

// Test-only conversion: production rejects every v2 snapshot. Historical inputs explicitly
// choose no physiology and the old zero minimum drafting gap, preserving the Dart oracle.
fn reference_fixture(source: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<Value>(source) else {
        return source.to_owned();
    };
    if v["schemaVersion"] != 2 {
        return source.to_owned();
    }
    v["schemaVersion"] = 3.into();
    if let Some(d) = v
        .pointer_mut("/config/drafting")
        .and_then(Value::as_object_mut)
    {
        let gap = if d["maxGapM"].is_null() {
            Value::Null
        } else {
            serde_json::json!(0.0)
        };
        d.insert("minGapM".into(), gap);
    }
    if let Some(riders) = v
        .pointer_mut("/config/riders")
        .and_then(Value::as_array_mut)
    {
        for rider in riders {
            rider["physiology"] = Value::Null;
        }
    }
    if let Some(riders) = v.pointer_mut("/state/riders").and_then(Value::as_array_mut) {
        for rider in riders {
            rider["wPrimeBalanceJ"] = Value::Null;
        }
    }
    v.to_string()
}
fn reference_projection(source: &str) -> String {
    source
        .replace("\"schemaVersion\":3", "\"schemaVersion\":2")
        .replace(",\"minGapM\":null", "")
        .replace(",\"minGapM\":0.0", "")
        .replace(",\"physiology\":null", "")
        .replace(",\"wPrimeBalanceJ\":null", "")
}

#[test]
fn production_rejects_all_historical_v2_snapshots() {
    for race in golden()["races"].as_array().unwrap() {
        let e = decode_snapshot(race["initialSnapshot"].as_str().unwrap()).unwrap_err();
        assert_eq!(
            e.decode_code(),
            Some(race_engine::SnapshotDecodeCode::UnsupportedSchemaVersion)
        );
    }
}
