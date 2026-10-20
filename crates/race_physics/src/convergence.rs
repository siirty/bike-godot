//! The deterministic convergence corpus and harness behind the fixed-timestep choice
//! (`--features testing`).
//!
//! One corpus and one harness back both the budget tests (`tests/convergence.rs`) and the README
//! table (`cargo run -p race_physics --features testing --example convergence_report`). A later
//! slice re-measuring the budget after adding pack dynamics extends this corpus rather than
//! writing a second one.

mod corpus;
mod harness;
mod report;
mod scenario;

pub use corpus::{
    convergence_corpus, CANDIDATE_STEPS, CONVERGENCE_ALL_ROUNDER, CONVERGENCE_CLIMBER,
    REFERENCE_STEP,
};
pub use harness::{
    compare_scenario, run_track, ScenarioError, TrackError, TrackTrajectory, COMPARISON_GRID,
};
pub use report::{format_significant, render_convergence_report};
pub use scenario::{ConvergenceScenario, InputSegment, RiderTrack};
