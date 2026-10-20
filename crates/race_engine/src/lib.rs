//! The deterministic live race kernel.
//!
//! One pure stepping kernel ([`RaceSimulation`]) with explicit state, inputs, time and
//! randomness. Headless tests and the live Godot adapter step the same object. Capability lives
//! in [`RaceSimulationConfig`]; only dynamic continuation data lives in [`RaceState`].

pub mod canonical_json;
#[cfg(feature = "testing")]
pub mod harness;
pub mod pack;
pub mod ride_session;
pub mod simulation;
pub mod tactics;
/// Tolerance-aware comparison for tests. Never used by the kernel.
#[cfg(feature = "testing")]
pub mod testing;

pub use ride_session::{
    RideAdvance, RideFrame, RideSession, RideSessionError, DEFAULT_RENDER_INTERVAL,
};
pub use simulation::contracts::{
    BasicDraftingConfig, RaceInputFrame, RaceSimulationConfig, RaceSimulationConfigSpec, RaceState,
    RaceStepResult, RiderControlInput, RiderDefinition, RiderState,
};
pub use simulation::error::{
    RaceSimulationError, Result, SimulationValidationCode, SnapshotDecodeCode, ROOT_PATH,
};
pub use simulation::events::{RaceEvent, RaceEventKind};
pub use simulation::kernel::RaceSimulation;
pub use simulation::pcg32::{hex64, parse_hex64, Pcg32, RandomState};
pub use simulation::rider_id::{canonical_rider_ids, RiderId};
pub use simulation::snapshot::{
    decode_snapshot, encode_snapshot, SimulationSnapshot, SNAPSHOT_FORMAT, SNAPSHOT_SCHEMA_VERSION,
};
pub use simulation::terrain::{
    BeyondEndBehavior, SimulationTerrain, TerrainInterpolation, TerrainSample,
};
