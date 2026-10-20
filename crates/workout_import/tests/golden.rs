//! Parity against vectors recorded from the Dart prototype (`fixtures/golden/workout_import.json`).
//!
//! Every expectation here is an output of the Dart reference, never of this crate. Doubles are
//! compared bit-for-bit: both sides perform the same IEEE-754 operations in the same order.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;
use workout_import::{
    decode_segments, encode_segments, parse_file, DecodedSegments, WorkoutTimeline,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden() -> Value {
    let path = repo_root().join("fixtures/golden/workout_import.json");
    serde_json::from_str(&std::fs::read_to_string(&path).expect("read golden vectors"))
        .expect("parse golden vectors")
}

/// The in-memory inputs the exporter used for files that are not in `fixtures/workouts`.
fn inline_source(name: &str) -> Option<&'static str> {
    Some(match name {
        "intervals.zwo" => include_str!("inline/intervals.zwo"),
        "crlf.erg" => {
            "[COURSE HEADER]\r\nVERSION = 2\r\nUNITS = ENGLISH\r\n\
            DESCRIPTION = CRLF test with tabs\r\nMINUTES WATTS\r\n[END COURSE HEADER]\r\n\
            [COURSE DATA]\r\n0.00\t100\r\n2.5\t100\r\n2.5 250\r\n4.25    250\r\n\
            4.25\t120\r\n10\t180\r\n[END COURSE DATA]\r\n"
        }
        "tabdesc.mrc" => {
            "[COURSE HEADER]\nDESCRIPTION\tTab separated\n[END COURSE HEADER]\n\
            [COURSE DATA]\n0 50\n1.5 75\n3 105.5\n[END COURSE DATA]\n"
        }
        "bad_root.zwo" => "<foo/>",
        "missing_name.zwo" => "<workout_file><workout/></workout_file>",
        "bad_number.zwo" => {
            "<workout_file><name>x</name><workout><SteadyState Duration=\"abc\" \
            Power=\"1\"/></workout></workout_file>"
        }
        "unknown_element.zwo" => {
            "<workout_file><name>x</name><workout><Teleport \
            Duration=\"1\"/></workout></workout_file>"
        }
        "no_power.zwo" => {
            "<workout_file><name>x</name><workout><SteadyState \
            Duration=\"10\"/></workout></workout_file>"
        }
        "no_data.erg" => "[COURSE HEADER]\nDESCRIPTION = x\n[END COURSE HEADER]\n",
        "one_point.erg" => "[COURSE DATA]\n0 100\n[END COURSE DATA]\n",
        "decreasing.mrc" => "[COURSE DATA]\n0 50\n2 60\n1 70\n[END COURSE DATA]\n",
        "three_tokens.erg" => "[COURSE DATA]\n0 100 5\n1 100\n[END COURSE DATA]\n",
        "unsupported.fit" => "whatever",
        _ => return None,
    })
}

fn source_bytes(name: &str) -> Vec<u8> {
    match inline_source(name) {
        Some(text) => text.as_bytes().to_vec(),
        None => std::fs::read(repo_root().join("fixtures/workouts").join(name))
            .unwrap_or_else(|e| panic!("read fixture {name}: {e}")),
    }
}

fn opt_str(v: &Value) -> Option<&str> {
    v.as_str()
}

fn bits(v: Option<f64>) -> Option<u64> {
    v.map(f64::to_bits)
}

#[test]
fn every_parse_matches_the_dart_reference() {
    let golden = golden();
    let parses = golden["parses"].as_array().unwrap();
    assert!(parses.len() >= 19, "vector file looks truncated");
    for case in parses {
        let name = case["fileName"].as_str().unwrap();
        let result = parse_file(name, &source_bytes(name));

        if case.get("error").is_some() {
            let err = result.expect_err(name);
            assert_eq!(
                err.message(),
                case["message"].as_str().unwrap(),
                "{name}: message"
            );
            assert_eq!(
                err.format(),
                opt_str(&case["errorFormat"]),
                "{name}: format tag"
            );
            continue;
        }

        let imported = result.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            imported.format.tag(),
            case["format"].as_str().unwrap(),
            "{name}"
        );
        let plan = &imported.plan;
        let expected = &case["plan"];
        assert_eq!(
            plan.name.as_deref(),
            opt_str(&expected["name"]),
            "{name}: name"
        );
        assert_eq!(
            plan.author.as_deref(),
            opt_str(&expected["author"]),
            "{name}: author"
        );
        assert_eq!(
            plan.description.as_deref(),
            opt_str(&expected["description"]),
            "{name}: description"
        );
        assert_eq!(
            plan.total_duration().as_micros() as i64,
            expected["totalDurationMicros"].as_i64().unwrap(),
            "{name}: total duration"
        );
        let encoded: Value = serde_json::from_str(&encode_segments(&plan.segments)).unwrap();
        assert_eq!(encoded, expected["segmentsJson"], "{name}: segments JSON");

        let timeline = WorkoutTimeline::new(plan.clone()).unwrap();
        for sample in case["timeline"].as_array().unwrap() {
            let t = Duration::from_micros(sample["t"].as_u64().unwrap());
            let at = sample["t"].as_i64().unwrap();
            assert_eq!(
                timeline.index_at(t).unwrap(),
                sample["index"].as_u64().unwrap() as usize,
                "{name} @ {at}: index"
            );
            assert_eq!(
                timeline.time_remaining_at(t).unwrap().as_micros() as i64,
                sample["remaining"].as_i64().unwrap(),
                "{name} @ {at}: remaining"
            );
            assert_eq!(
                timeline.time_elapsed_in_segment_at(t).unwrap().as_micros() as i64,
                sample["elapsedInSegment"].as_i64().unwrap(),
                "{name} @ {at}: elapsed in segment"
            );
            for (ftp, key) in [(250.0, "power250"), (312.5, "power312")] {
                assert_eq!(
                    bits(timeline.power_at(t, ftp).unwrap()),
                    bits(sample[key].as_f64()),
                    "{name} @ {at}: {key}"
                );
            }
        }
    }
}

#[test]
fn legacy_blobs_decode_and_reencode_like_dart() {
    let golden = golden();
    for case in golden["legacy"].as_array().unwrap() {
        let file = case["file"].as_str().unwrap();
        let source = std::fs::read_to_string(
            repo_root()
                .join("fixtures/workouts/legacy_segments_json")
                .join(file),
        )
        .unwrap();
        match decode_segments(&source) {
            DecodedSegments::Readable { segments, version } => {
                assert!(case["readable"].as_bool().unwrap(), "{file}");
                assert_eq!(version, case["version"].as_i64().unwrap(), "{file}");
                let reencoded: Value = serde_json::from_str(&encode_segments(&segments)).unwrap();
                assert_eq!(reencoded, case["reencoded"], "{file}");
            }
            DecodedSegments::Unreadable { reason, .. } => panic!("{file}: {reason}"),
        }
    }
}

#[test]
fn decode_classification_matches_dart() {
    let golden = golden();
    for case in golden["decodes"].as_array().unwrap() {
        let source = case["source"].as_str().unwrap();
        match decode_segments(source) {
            DecodedSegments::Readable { segments, version } => {
                assert!(
                    case["readable"].as_bool().unwrap(),
                    "{source:?} decoded but Dart refused"
                );
                assert_eq!(
                    Some(version),
                    case["version"].as_i64(),
                    "{source:?}: version"
                );
                let reencoded: Value = serde_json::from_str(&encode_segments(&segments)).unwrap();
                assert_eq!(reencoded, case["reencoded"], "{source:?}");
            }
            DecodedSegments::Unreadable {
                kind,
                version,
                reason,
            } => {
                assert!(
                    !case["readable"].as_bool().unwrap(),
                    "{source:?} refused ({reason}) but Dart decoded it"
                );
                let kind = match kind {
                    workout_import::UnreadableReason::Malformed => "malformed",
                    workout_import::UnreadableReason::TooNew => "tooNew",
                };
                assert_eq!(kind, case["kind"].as_str().unwrap(), "{source:?}: kind");
                // Known divergence (docs/port/divergences.md): when `segments` is not a list the
                // prototype drops the envelope version it had already read, contradicting its own
                // documented contract. This port reports it.
                let expected_version = if source == r#"{"version":1,"segments":"not a list"}"# {
                    Some(1)
                } else {
                    case["version"].as_i64()
                };
                assert_eq!(version, expected_version, "{source:?}: version");
            }
        }
    }
}
