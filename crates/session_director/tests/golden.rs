//! Parity against `fixtures/golden/session_director.json`, recorded from the Dart prototype.

use std::path::PathBuf;

use race_physics::RiderPhysicalParams;
use serde_json::Value;
use session_director::{
    grade_for_effort, zone_for_effort, GradeForPowerSynthesis, RiderProfile, TerrainSource,
    TerrainSynthesisStrategy,
};
use workout_import::parse_file;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/golden/session_director.json")).unwrap(),
    )
    .unwrap()
}

fn profiles() -> [RiderProfile; 2] {
    [
        RiderProfile::new(250.0, RiderPhysicalParams::new(78.0)),
        RiderProfile::new(
            315.0,
            RiderPhysicalParams {
                cda_m2: 0.27,
                ..RiderPhysicalParams::new(68.0)
            },
        ),
    ]
}

fn workout_bytes(name: &str) -> Vec<u8> {
    if name == "intervals.zwo" {
        std::fs::read(root().join("crates/workout_import/tests/inline/intervals.zwo")).unwrap()
    } else {
        std::fs::read(root().join("fixtures/workouts").join(name)).unwrap()
    }
}

#[track_caller]
fn same(actual: f64, expected: &Value, ctx: &str) {
    let e = expected.as_f64().unwrap();
    assert_eq!(
        actual.to_bits(),
        e.to_bits(),
        "{ctx}: rust {actual:e} vs dart {e:e}"
    );
}

#[test]
fn synthesis_matches_bit_for_bit() {
    let g = golden();
    let cases = g["syntheses"].as_array().unwrap();
    assert_eq!(cases.len(), 14);
    for case in cases {
        let name = case["workout"].as_str().unwrap();
        let p = case["profile"].as_u64().unwrap() as usize;
        let ctx = format!("{name} / profile {p}");
        let plan = parse_file(name, &workout_bytes(name)).unwrap().plan;
        let terrain = GradeForPowerSynthesis
            .synthesize(&plan, &profiles()[p], 7)
            .unwrap();

        let samples = case["samples"].as_array().unwrap();
        assert_eq!(
            terrain.samples().len(),
            samples.len(),
            "{ctx}: sample count"
        );
        for (s, e) in terrain.samples().iter().zip(samples) {
            same(s.distance_m, &e[0], &ctx);
            same(s.grade_fraction, &e[1], &ctx);
        }
        let ranges = case["ranges"].as_array().unwrap();
        assert_eq!(terrain.segment_ranges().len(), ranges.len(), "{ctx}");
        for (r, e) in terrain.segment_ranges().iter().zip(ranges) {
            same(r.start_distance_m, &e[0], &ctx);
            same(r.end_distance_m, &e[1], &ctx);
        }
        same(terrain.length_m().unwrap(), &case["lengthM"], &ctx);
        let length = terrain.length_m().unwrap();
        for (k, probe) in case["probes"].as_array().unwrap().iter().enumerate() {
            let i = k as f64 - 1.0;
            let d = length * i / 397.0;
            same(d, &probe[0], &ctx);
            same(
                terrain.grade_at_distance(d),
                &probe[1],
                &format!("{ctx} @ {d}"),
            );
        }
    }
}

#[test]
fn effort_to_grade_and_zone_match() {
    let g = golden();
    for case in g["gradeForEffort"].as_array().unwrap() {
        let effort = case["effort"].as_f64().unwrap();
        same(
            grade_for_effort(effort),
            &case["grade"],
            &format!("effort {effort}"),
        );
        assert_eq!(
            zone_for_effort(effort).name,
            case["zone"],
            "effort {effort}"
        );
    }
}
