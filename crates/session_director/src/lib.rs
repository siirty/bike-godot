//! Deterministic pre-ride planning: a workout plan and rider profile become a frozen terrain.
//!
//! Pure and deterministic for fixed inputs and seed: no wall clock, no live rider state, no I/O.
//! It scripts the opportunity, not the outcome. It never precomputes AI trajectories, and it
//! never consumes live race state. The planner proper (training contracts, race scenarios, scene
//! goals) is later-slice work; what exists today is the terrain seam the ride loop consumes.

mod profile;
mod synthesis;
mod terrain;
mod zones;

pub use profile::RiderProfile;
pub use synthesis::{
    grade_for_effort, segment_target_watts, GradeForPowerSynthesis, SynthesisError,
    TerrainSynthesisStrategy,
};
pub use terrain::{
    FlatTerrain, GradePoint, SegmentDistanceRange, SynthesizedTerrain, TerrainSource,
};
pub use zones::{zone_for_effort, Rgb, WorkloadZone, ALL_WORKLOAD_ZONES};
