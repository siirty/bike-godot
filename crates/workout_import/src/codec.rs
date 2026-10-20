//! The persisted JSON shape of a workout's segments. This crate owns it; storage only maps a column.
//!
//! ```json
//! {"version": 1, "segments": [ ... ]}
//! ```
//!
//! The key names, the `"type"` discriminator values and whole-millisecond durations are the
//! on-disk format the Dart prototype wrote; rows it stored must read back identically, so this
//! module spells them out explicitly rather than leaving them to a derive.

use std::time::Duration;

use serde_json::{json, Map, Value};

use crate::model::{IntervalRepeat, IntervalRepeatSpec, PowerTarget, WorkoutSegment};

/// The envelope version this build writes.
pub const CURRENT_VERSION: i64 = 1;

/// Encodes `segments` as a versioned envelope.
pub fn encode_segments(segments: &[WorkoutSegment]) -> String {
    let segments: Vec<Value> = segments.iter().map(segment_to_json).collect();
    json!({ "version": CURRENT_VERSION, "segments": segments }).to_string()
}

/// Why a stored blob could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreadableReason {
    /// Damaged, truncated, hand-edited, or never a shape this format had.
    Malformed,
    /// Well-formed but written by a newer build. Must not be presented as corruption.
    TooNew,
}

/// The outcome of [`decode_segments`]. Decoding never fails outright: a caller holding a row it
/// cannot read still has the row, and can tell the rider whether it is damaged or merely too new.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedSegments {
    /// Read successfully. `version` 0 means a legacy bare array that predates the envelope.
    Readable {
        /// The segments.
        segments: Vec<WorkoutSegment>,
        /// The envelope version the blob carried.
        version: i64,
    },
    /// Could not be read.
    Unreadable {
        /// A diagnostic explanation (not for showing to a rider verbatim).
        reason: String,
        /// Why.
        kind: UnreadableReason,
        /// The envelope version, if one could be read.
        version: Option<i64>,
    },
}

fn malformed(reason: impl Into<String>, version: Option<i64>) -> DecodedSegments {
    DecodedSegments::Unreadable {
        reason: reason.into(),
        kind: UnreadableReason::Malformed,
        version,
    }
}

/// Decodes a stored blob: the current envelope, a legacy bare array (version 0), or anything else
/// as [`DecodedSegments::Unreadable`].
pub fn decode_segments(source: &str) -> DecodedSegments {
    let root: Value = match serde_json::from_str(source) {
        Ok(v) => v,
        Err(e) => return malformed(format!("not valid JSON: {e}"), None),
    };

    let (raw_segments, version) = match &root {
        Value::Array(_) => (&root, 0),
        Value::Object(map) => {
            let Some(version) = map.get("version").and_then(Value::as_i64) else {
                return malformed("envelope is missing an integer \"version\"", None);
            };
            if version > CURRENT_VERSION {
                return DecodedSegments::Unreadable {
                    reason: format!(
                        "stored with format version {version}, but this build understands up to \
                         {CURRENT_VERSION}"
                    ),
                    kind: UnreadableReason::TooNew,
                    version: Some(version),
                };
            }
            (map.get("segments").unwrap_or(&Value::Null), version)
        }
        _ => return malformed("expected a JSON object or array at the top level", None),
    };

    let Value::Array(raw_segments) = raw_segments else {
        return malformed("\"segments\" is not a JSON array", Some(version));
    };

    let mut segments = Vec::with_capacity(raw_segments.len());
    for (i, raw) in raw_segments.iter().enumerate() {
        let Value::Object(object) = raw else {
            return malformed(format!("segment {i} is not a JSON object"), Some(version));
        };
        match segment_from_json(object) {
            Ok(segment) => segments.push(segment),
            Err(e) => {
                return malformed(format!("segment {i} could not be read: {e}"), Some(version))
            }
        }
    }
    DecodedSegments::Readable { segments, version }
}

/// One segment's JSON object, in the persisted shape.
pub fn segment_to_json(segment: &WorkoutSegment) -> Value {
    let mut object = match segment {
        WorkoutSegment::SteadyState {
            duration,
            target_power,
            cadence,
        } => json!({
            "durationMs": duration_ms(*duration),
            "targetPower": power_to_json(*target_power),
            "cadence": cadence,
        }),
        WorkoutSegment::Ramp {
            duration,
            start_power,
            end_power,
            cadence,
        } => json!({
            "durationMs": duration_ms(*duration),
            "startPower": power_to_json(*start_power),
            "endPower": power_to_json(*end_power),
            "cadence": cadence,
        }),
        WorkoutSegment::IntervalRepeat(block) => json!({
            "repeat": block.repeat(),
            "onDurationMs": duration_ms(block.on_duration()),
            "onStartPower": power_to_json(block.on_start_power()),
            "onEndPower": power_to_json(block.on_end_power()),
            "offDurationMs": duration_ms(block.off_duration()),
            "offStartPower": block.off_start_power().map(power_to_json),
            "offEndPower": block.off_end_power().map(power_to_json),
            "onCadence": block.on_cadence(),
            "offCadence": block.off_cadence(),
        }),
        WorkoutSegment::FreeRide { duration, cadence } => json!({
            "durationMs": duration_ms(*duration),
            "cadence": cadence,
        }),
    };
    object["type"] = Value::from(segment.kind_tag());
    object
}

/// One power target's JSON object, in the persisted shape.
pub fn power_to_json(power: PowerTarget) -> Value {
    match power {
        PowerTarget::PercentFtp(fraction) => json!({ "type": "percentFtp", "fraction": fraction }),
        PowerTarget::AbsoluteWatts(watts) => json!({ "type": "absoluteWatts", "watts": watts }),
    }
}

fn duration_ms(d: Duration) -> i64 {
    // Milliseconds, truncated — Dart's `Duration.inMilliseconds`. Workout files express lengths in
    // whole seconds, so nothing is lost in practice.
    i64::try_from(d.as_millis()).expect("segment durations fit in i64 milliseconds")
}

/// Reads one segment back from its persisted shape, validating the model's invariants.
pub fn segment_from_json(object: &Map<String, Value>) -> Result<WorkoutSegment, String> {
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or("missing string \"type\"")?;
    match kind {
        "steadyState" => Ok(WorkoutSegment::SteadyState {
            duration: required_duration(object, "durationMs")?,
            target_power: required_power(object, "targetPower")?,
            cadence: optional_int(object, "cadence")?,
        }),
        "ramp" => Ok(WorkoutSegment::Ramp {
            duration: required_duration(object, "durationMs")?,
            start_power: required_power(object, "startPower")?,
            end_power: required_power(object, "endPower")?,
            cadence: optional_int(object, "cadence")?,
        }),
        "intervalRepeat" => {
            let repeat = required_int(object, "repeat")?;
            let repeat = u32::try_from(repeat)
                .map_err(|_| format!("\"repeat\" must be a positive count, found {repeat}"))?;
            let off_duration = match object.get("offDurationMs") {
                None | Some(Value::Null) => Duration::ZERO,
                Some(_) => required_duration(object, "offDurationMs")?,
            };
            IntervalRepeat::new(IntervalRepeatSpec {
                repeat,
                on_duration: required_duration(object, "onDurationMs")?,
                on_start_power: required_power(object, "onStartPower")?,
                on_end_power: required_power(object, "onEndPower")?,
                off_duration,
                off_start_power: optional_power(object, "offStartPower")?,
                off_end_power: optional_power(object, "offEndPower")?,
                on_cadence: optional_int(object, "onCadence")?,
                off_cadence: optional_int(object, "offCadence")?,
            })
            .map(WorkoutSegment::IntervalRepeat)
            .map_err(|e| format!("invalid: {}", e.message()))
        }
        "freeRide" => Ok(WorkoutSegment::FreeRide {
            duration: required_duration(object, "durationMs")?,
            cadence: optional_int(object, "cadence")?,
        }),
        other => Err(format!("unknown segment type \"{other}\"")),
    }
}

/// Reads one power target back from its persisted shape.
pub fn power_from_json(value: &Value) -> Result<PowerTarget, String> {
    let object = value.as_object().ok_or("power target is not an object")?;
    match object.get("type").and_then(Value::as_str) {
        Some("percentFtp") => Ok(PowerTarget::PercentFtp(required_f64(object, "fraction")?)),
        Some("absoluteWatts") => Ok(PowerTarget::AbsoluteWatts(required_f64(object, "watts")?)),
        Some(other) => Err(format!("unknown power target type \"{other}\"")),
        None => Err("power target is missing a string \"type\"".to_owned()),
    }
}

fn required_f64(object: &Map<String, Value>, key: &str) -> Result<f64, String> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("\"{key}\" must be a number"))
}

/// An integer field. Dart's generated reader did `(json[k] as num).toInt()`, so an integral double
/// such as `90.0` is accepted too.
fn as_int(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        let f = value.as_f64()?;
        (f.is_finite() && f.fract() == 0.0 && f.abs() < 9.0e15).then_some(f as i64)
    })
}

fn required_int(object: &Map<String, Value>, key: &str) -> Result<i64, String> {
    object
        .get(key)
        .and_then(as_int)
        .ok_or_else(|| format!("\"{key}\" must be an integer"))
}

fn optional_int(object: &Map<String, Value>, key: &str) -> Result<Option<i64>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => as_int(v)
            .map(Some)
            .ok_or_else(|| format!("\"{key}\" must be an integer or null")),
    }
}

fn required_duration(object: &Map<String, Value>, key: &str) -> Result<Duration, String> {
    let ms = required_int(object, key)?;
    let ms = u64::try_from(ms).map_err(|_| format!("\"{key}\" must not be negative"))?;
    Ok(Duration::from_millis(ms))
}

fn required_power(object: &Map<String, Value>, key: &str) -> Result<PowerTarget, String> {
    power_from_json(
        object
            .get(key)
            .ok_or_else(|| format!("missing \"{key}\""))?,
    )
    .map_err(|e| format!("\"{key}\": {e}"))
}

fn optional_power(object: &Map<String, Value>, key: &str) -> Result<Option<PowerTarget>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => power_from_json(v)
            .map(Some)
            .map_err(|e| format!("\"{key}\": {e}")),
    }
}
