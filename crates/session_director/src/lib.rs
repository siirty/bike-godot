//! Deterministic pre-ride planning: a workout plan and rider profile become a frozen terrain.
//!
//! Pure and deterministic for fixed inputs and seed: no wall clock, no live rider state, no I/O.
//! It scripts the opportunity, not the outcome. It never precomputes AI trajectories, and it
//! never consumes live race state. The planner proper (training contracts, race scenarios, scene
//! goals) is still incomplete. Validated learning-situation and player-knowledge contracts are
//! available alongside the terrain seam; general workout compilation remains future work.

pub mod mixed;
mod profile;
pub mod situations;
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
