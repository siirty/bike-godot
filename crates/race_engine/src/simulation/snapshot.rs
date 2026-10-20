//! Self-contained, versioned snapshots as canonical UTF-8 JSON.
//!
//! A resume point with a lifetime of hours, not a history format: no migration, and an unknown
//! schema version is rejected rather than reinterpreted. No wall-clock value is ever written, so
//! the same simulation always encodes to the same bytes, byte-identical to the Dart prototype.
//! Storage is someone else's job: this produces and consumes a `String`.

use std::time::Duration;

use race_physics::{LongitudinalDynamicsConfig, PhysicsError, RiderPhysicalParams};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::dart_json::to_dart_json;
use crate::simulation::error::{
    at, RaceSimulationError, Result, SimulationValidationCode, SnapshotDecodeCode as D, ROOT_PATH,
};
use crate::simulation::pcg32::{hex64, parse_hex64};
use crate::{
    BasicDraftingConfig, BeyondEndBehavior, RaceSimulationConfig, RaceSimulationConfigSpec,
    RaceState, RandomState, RiderDefinition, RiderId, RiderState, SimulationTerrain,
    TerrainInterpolation, TerrainSample,
};

/// The `format` string every snapshot begins with.
pub const SNAPSHOT_FORMAT: &str = "bike-training-race-snapshot";

/// The schema version this build writes and reads. v2 added `config.dynamics` (and carries
/// `config.drafting`, `null` for a modelled absence).
pub const SNAPSHOT_SCHEMA_VERSION: i64 = 2;

/// A decoded pair, validated against each other.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationSnapshot {
    config: RaceSimulationConfig,
    state: RaceState,
}

impl SimulationSnapshot {
    /// Validates `state` against `config`.
    pub fn new(config: RaceSimulationConfig, state: RaceState) -> Result<Self> {
        config.validate_state(&state, "state")?;
        Ok(Self { config, state })
    }

    /// What does not change.
    pub fn config(&self) -> &RaceSimulationConfig {
        &self.config
    }

    /// Where the race had got to.
    pub fn state(&self) -> &RaceState {
        &self.state
    }

    /// Both halves.
    pub fn into_parts(self) -> (RaceSimulationConfig, RaceState) {
        (self.config, self.state)
    }
}

// --- Encoding: field order here is the documented canonical order. ------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotJson<'a> {
    format: &'static str,
    schema_version: i64,
    config: ConfigJson<'a>,
    state: StateJson<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigJson<'a> {
    fixed_step_micros: i64,
    lateral_slot_count: i64,
    air_density_kg_m3: f64,
    headwind_ms: f64,
    dynamics: DynamicsJson,
    drafting: DraftingJson<'a>,
    terrain: TerrainJson,
    riders: Vec<RiderDefinitionJson<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DynamicsJson {
    max_propulsive_force_n: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DraftingJson<'a> {
    max_gap_m: Option<f64>,
    minimum_drag_multiplier_by_slot_offset: Option<&'a [f64]>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainJson {
    interpolation: &'static str,
    beyond_end: &'static str,
    length_m: Option<f64>,
    samples: Vec<SampleJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SampleJson {
    distance_m: f64,
    grade_fraction: f64,
}

#[derive(Serialize)]
struct RiderDefinitionJson<'a> {
    id: &'a str,
    /// `race_physics` owns this mapping; the codec only places it.
    physical: &'a RiderPhysicalParams,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateJson<'a> {
    simulation_time_micros: i64,
    random: RandomJson,
    riders: Vec<RiderStateJson<'a>>,
}

#[derive(Serialize)]
struct RandomJson {
    algorithm: &'static str,
    state: String,
    stream: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RiderStateJson<'a> {
    id: &'a str,
    distance_m: f64,
    speed_ms: f64,
    lateral_slot: i64,
    draft_target_id: Option<&'a str>,
}

fn micros(d: Duration) -> i64 {
    i64::try_from(d.as_micros()).expect("simulation durations fit in i64 microseconds")
}

/// Encodes a canonical snapshot. `state` is validated against `config` first, so an unresumable
/// snapshot cannot be written.
pub fn encode_snapshot(config: &RaceSimulationConfig, state: &RaceState) -> Result<String> {
    config.validate_state(state, "state")?;
    let terrain = config.terrain();
    let doc = SnapshotJson {
        format: SNAPSHOT_FORMAT,
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        config: ConfigJson {
            fixed_step_micros: micros(config.fixed_step()),
            lateral_slot_count: config.lateral_slot_count() as i64,
            air_density_kg_m3: config.air_density_kg_m3(),
            headwind_ms: config.headwind_ms(),
            dynamics: DynamicsJson {
                max_propulsive_force_n: config.dynamics().max_propulsive_force_n(),
            },
            drafting: DraftingJson {
                max_gap_m: config.drafting().map(BasicDraftingConfig::max_gap_m),
                minimum_drag_multiplier_by_slot_offset: config
                    .drafting()
                    .map(BasicDraftingConfig::minimum_drag_multiplier_by_slot_offset),
            },
            terrain: TerrainJson {
                interpolation: terrain.interpolation().name(),
                beyond_end: terrain.beyond_end().name(),
                length_m: terrain.explicit_length_m(),
                samples: terrain
                    .samples()
                    .iter()
                    .map(|s| SampleJson {
                        distance_m: s.distance_m(),
                        grade_fraction: s.grade_fraction(),
                    })
                    .collect(),
            },
            riders: config
                .riders()
                .iter()
                .map(|r| RiderDefinitionJson {
                    id: r.id().as_str(),
                    physical: r.physical(),
                })
                .collect(),
        },
        state: StateJson {
            simulation_time_micros: micros(state.simulation_time()),
            random: RandomJson {
                algorithm: state.random_state().algorithm(),
                state: hex64(state.random_state().state()),
                stream: hex64(state.random_state().stream()),
            },
            riders: state
                .riders()
                .iter()
                .map(|r| RiderStateJson {
                    id: r.id().as_str(),
                    distance_m: r.distance_m(),
                    speed_ms: r.speed_ms(),
                    lateral_slot: r.lateral_slot() as i64,
                    draft_target_id: r.draft_target_id().map(RiderId::as_str),
                })
                .collect(),
        },
    };
    Ok(to_dart_json(&doc))
}

// --- Decoding: strict, typed, with full paths. Nothing is returned until everything is checked. --

/// Decodes and fully validates a snapshot.
///
/// [`RaceSimulationError::Decode`] when the bytes are not a snapshot this build reads;
/// [`RaceSimulationError::Validation`] when they are, but describe an illegal simulation.
pub fn decode_snapshot(source: &str) -> Result<SimulationSnapshot> {
    let root: Value = serde_json::from_str(source).map_err(|e| {
        RaceSimulationError::decode(D::MalformedJson, format!("not valid JSON: {e}"), ROOT_PATH)
    })?;
    let map = as_map(&root, ROOT_PATH)?;
    let format = string(map, "format", ROOT_PATH)?;
    if format != SNAPSHOT_FORMAT {
        return Err(RaceSimulationError::decode(
            D::UnsupportedFormat,
            format!("expected format \"{SNAPSHOT_FORMAT}\", got \"{format}\""),
            "format",
        ));
    }
    let version = int(map, "schemaVersion", ROOT_PATH)?;
    if version != SNAPSHOT_SCHEMA_VERSION {
        return Err(RaceSimulationError::decode(
            D::UnsupportedSchemaVersion,
            format!(
                "snapshot schema version {version} is not readable by this build, which reads \
                 version {SNAPSHOT_SCHEMA_VERSION}"
            ),
            "schemaVersion",
        ));
    }
    let config = decode_config(
        as_map(required(map, "config", ROOT_PATH)?, "config")?,
        "config",
    )?;
    let state = decode_state(
        as_map(required(map, "state", ROOT_PATH)?, "state")?,
        "state",
    )?;
    SimulationSnapshot::new(config, state)
}

fn decode_config(json: &Map<String, Value>, path: &str) -> Result<RaceSimulationConfig> {
    let fixed_step_micros = int(json, "fixedStepMicros", path)?;
    let terrain_path = format!("{path}.terrain");
    let terrain = decode_terrain(
        as_map(required(json, "terrain", path)?, &terrain_path)?,
        &terrain_path,
    )?;
    let riders = list(json, "riders", path)?;
    let dynamics_path = format!("{path}.dynamics");
    let dynamics_map = as_map(required(json, "dynamics", path)?, &dynamics_path)?;
    let drafting_path = format!("{path}.drafting");
    let drafting_map = as_map(required(json, "drafting", path)?, &drafting_path)?;
    let max_gap_m = nullable_double(drafting_map, "maxGapM", &drafting_path)?;
    let multipliers_path = format!("{drafting_path}.minimumDragMultiplierBySlotOffset");
    let multipliers = match drafting_map.get("minimumDragMultiplierBySlotOffset") {
        None | Some(Value::Null) => None,
        Some(Value::Array(values)) => Some(
            values
                .iter()
                .enumerate()
                .map(|(i, v)| number(v, &format!("{multipliers_path}[{i}]")))
                .collect::<Result<Vec<_>>>()?,
        ),
        Some(other) => {
            return Err(RaceSimulationError::decode(
                D::WrongType,
                format!(
                    "expected a JSON array of numbers or null, got {}",
                    type_name(other)
                ),
                multipliers_path,
            ))
        }
    };
    if max_gap_m.is_none() != multipliers.is_none() {
        return Err(RaceSimulationError::decode(
            D::MalformedJson,
            format!(
                "drafting must be entirely present or entirely null: maxGapM is {} but the \
                 multiplier list is {}",
                if max_gap_m.is_none() { "null" } else { "set" },
                if multipliers.is_none() { "null" } else { "set" }
            ),
            drafting_path,
        ));
    }

    let mut definitions = Vec::with_capacity(riders.len());
    for (i, raw) in riders.iter().enumerate() {
        let rider_path = format!("{path}.riders[{i}]");
        definitions.push(decode_rider_definition(
            as_map(raw, &rider_path)?,
            &rider_path,
            i,
            path,
        )?);
    }
    let max_propulsive_force_n = double(dynamics_map, "maxPropulsiveForceN", &dynamics_path)?;
    let dynamics = LongitudinalDynamicsConfig::new(max_propulsive_force_n).map_err(|e| {
        let PhysicsError::InvalidArgument { message, .. } = e else {
            unreachable!("config construction only validates its argument")
        };
        RaceSimulationError::validation(
            SimulationValidationCode::NotPositive,
            format!("maxPropulsiveForceN {message}"),
            format!("{dynamics_path}.maxPropulsiveForceN"),
        )
    })?;
    let drafting = match (max_gap_m, multipliers) {
        (Some(gap), Some(list)) => {
            Some(at(&drafting_path, || BasicDraftingConfig::new(gap, list))?)
        }
        _ => None,
    };
    let lateral_slot_count = int(json, "lateralSlotCount", path)?;
    let air_density_kg_m3 = double(json, "airDensityKgM3", path)?;
    let headwind_ms = double(json, "headwindMs", path)?;
    if fixed_step_micros < 0 {
        return Err(RaceSimulationError::validation(
            SimulationValidationCode::NotPositive,
            format!("expected a positive duration, got {fixed_step_micros} µs"),
            format!("{path}.fixedStep"),
        ));
    }
    at(path, || {
        RaceSimulationConfig::new(RaceSimulationConfigSpec {
            fixed_step: Duration::from_micros(fixed_step_micros as u64),
            riders: definitions,
            terrain,
            dynamics,
            drafting,
            lateral_slot_count,
            air_density_kg_m3,
            headwind_ms,
        })
    })
}

fn decode_terrain(json: &Map<String, Value>, path: &str) -> Result<SimulationTerrain> {
    let interpolation_name = string(json, "interpolation", path)?;
    let interpolation = TerrainInterpolation::by_name(interpolation_name).ok_or_else(|| {
        RaceSimulationError::decode(
            D::UnknownEnumValue,
            format!(
                "unknown terrain interpolation \"{interpolation_name}\"; this build knows [{}]",
                TerrainInterpolation::ALL
                    .map(TerrainInterpolation::name)
                    .join(", ")
            ),
            format!("{path}.interpolation"),
        )
    })?;
    let beyond_end_name = string(json, "beyondEnd", path)?;
    let beyond_end = BeyondEndBehavior::by_name(beyond_end_name).ok_or_else(|| {
        RaceSimulationError::decode(
            D::UnknownEnumValue,
            format!(
                "unknown beyond-end behaviour \"{beyond_end_name}\"; this build knows [{}]",
                BeyondEndBehavior::ALL
                    .map(BeyondEndBehavior::name)
                    .join(", ")
            ),
            format!("{path}.beyondEnd"),
        )
    })?;
    let length_m = nullable_double(json, "lengthM", path)?;
    let raw_samples = list(json, "samples", path)?;
    let mut samples = Vec::with_capacity(raw_samples.len());
    for (i, raw) in raw_samples.iter().enumerate() {
        let sample_path = format!("{path}.samples[{i}]");
        let sample = as_map(raw, &sample_path)?;
        let distance = double(sample, "distanceM", &sample_path)?;
        let grade = double(sample, "gradeFraction", &sample_path)?;
        samples.push(at(&sample_path, || TerrainSample::new(distance, grade))?);
    }
    at(path, || {
        SimulationTerrain::new(samples, interpolation, beyond_end, length_m)
    })
}

fn decode_rider_definition(
    json: &Map<String, Value>,
    path: &str,
    index: usize,
    config_path: &str,
) -> Result<RiderDefinition> {
    let physical_path = format!("{path}.physical");
    let physical_map = as_map(required(json, "physical", path)?, &physical_path)?;
    // Strict: every key race_physics writes must be present, because the encoder wrote them all;
    // an absent one is damage, not "use the default".
    let reference = serde_json::to_value(RiderPhysicalParams::new(1.0)).expect("plain data");
    for key in reference.as_object().expect("an object").keys() {
        double(physical_map, key, &physical_path)?;
    }
    let physical: RiderPhysicalParams = serde_json::from_value(Value::Object(physical_map.clone()))
        .map_err(|e| {
            RaceSimulationError::decode(D::WrongType, e.to_string(), physical_path.clone())
        })?;
    let raw_id = string(json, "id", path)?;
    let prefix = format!("{config_path}.riders[{index}]");
    let id = at(&prefix, || at("id", || RiderId::new(raw_id)))?;
    at(&prefix, || RiderDefinition::new(id, physical))
}

fn decode_state(json: &Map<String, Value>, path: &str) -> Result<RaceState> {
    let random_path = format!("{path}.random");
    let random = as_map(required(json, "random", path)?, &random_path)?;
    let raw_riders = list(json, "riders", path)?;

    let algorithm = string(random, "algorithm", &random_path)?;
    let rng_state = hex_field(random, "state", &random_path)?;
    let rng_stream = hex_field(random, "stream", &random_path)?;
    let random_state = at(&random_path, || {
        RandomState::new(algorithm, rng_state, rng_stream)
    })?;

    let mut riders = Vec::with_capacity(raw_riders.len());
    for (i, raw) in raw_riders.iter().enumerate() {
        let rider_path = format!("{path}.riders[{i}]");
        let rider = as_map(raw, &rider_path)?;
        let id = rider_id(rider, "id", &rider_path)?;
        let distance_m = double(rider, "distanceM", &rider_path)?;
        let speed_ms = double(rider, "speedMs", &rider_path)?;
        let lateral_slot = int(rider, "lateralSlot", &rider_path)?;
        let draft_target_id = match required(rider, "draftTargetId", &rider_path)? {
            Value::Null => None,
            _ => Some(rider_id(rider, "draftTargetId", &rider_path)?),
        };
        riders.push(at(&rider_path, || {
            RiderState::new(id, distance_m, speed_ms, lateral_slot, draft_target_id)
        })?);
    }
    let time_micros = int(json, "simulationTimeMicros", path)?;
    if time_micros < 0 {
        return Err(RaceSimulationError::validation(
            SimulationValidationCode::NegativeValue,
            format!("expected a non-negative duration, got {time_micros} µs"),
            format!("{path}.simulationTime"),
        ));
    }
    at(path, || {
        RaceState::new(
            Duration::from_micros(time_micros as u64),
            random_state,
            riders,
        )
    })
}

// --- Typed field readers. -----------------------------------------------------------------------

fn join(path: &str, key: &str) -> String {
    if path == ROOT_PATH {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::String(_) => "a string",
        Value::Number(n) if n.is_f64() => "a number",
        Value::Number(_) => "an integer",
        Value::Bool(_) => "a boolean",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn required<'a>(json: &'a Map<String, Value>, key: &str, path: &str) -> Result<&'a Value> {
    json.get(key).ok_or_else(|| {
        RaceSimulationError::decode(
            D::MissingField,
            format!("missing required field \"{key}\""),
            join(path, key),
        )
    })
}

fn as_map<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        RaceSimulationError::decode(
            D::WrongType,
            format!("expected a JSON object, got {}", type_name(value)),
            path,
        )
    })
}

fn list<'a>(json: &'a Map<String, Value>, key: &str, path: &str) -> Result<&'a Vec<Value>> {
    let value = required(json, key, path)?;
    value.as_array().ok_or_else(|| {
        RaceSimulationError::decode(
            D::WrongType,
            format!("expected a JSON array, got {}", type_name(value)),
            join(path, key),
        )
    })
}

fn string<'a>(json: &'a Map<String, Value>, key: &str, path: &str) -> Result<&'a str> {
    let value = required(json, key, path)?;
    value.as_str().ok_or_else(|| {
        RaceSimulationError::decode(
            D::WrongType,
            format!("expected a string, got {}", type_name(value)),
            join(path, key),
        )
    })
}

/// A JSON integer (no fraction or exponent), as the prototype's `jsonDecode` reads an `int`.
fn int(json: &Map<String, Value>, key: &str, path: &str) -> Result<i64> {
    let value = required(json, key, path)?;
    value.as_i64().ok_or_else(|| {
        RaceSimulationError::decode(
            D::WrongType,
            format!("expected an integer, got {}", type_name(value)),
            join(path, key),
        )
    })
}

/// Any JSON number as a double: `0` for `0.0` must decode.
fn number(value: &Value, path: &str) -> Result<f64> {
    match value {
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| RaceSimulationError::decode(D::WrongType, "expected a number", path)),
        other => Err(RaceSimulationError::decode(
            D::WrongType,
            format!("expected a number, got {}", type_name(other)),
            path,
        )),
    }
}

fn double(json: &Map<String, Value>, key: &str, path: &str) -> Result<f64> {
    number(required(json, key, path)?, &join(path, key))
}

fn nullable_double(json: &Map<String, Value>, key: &str, path: &str) -> Result<Option<f64>> {
    match required(json, key, path)? {
        Value::Null => Ok(None),
        v => number(v, &join(path, key)).map(Some),
    }
}

fn rider_id(json: &Map<String, Value>, key: &str, path: &str) -> Result<RiderId> {
    let raw = string(json, key, path)?;
    at(key, || RiderId::new(raw))
}

fn hex_field(json: &Map<String, Value>, key: &str, path: &str) -> Result<u64> {
    let raw = string(json, key, path)?;
    parse_hex64(raw).ok_or_else(|| {
        RaceSimulationError::decode(
            D::WrongType,
            format!("expected exactly 16 lowercase hexadecimal digits, got \"{raw}\""),
            join(path, key),
        )
    })
}
