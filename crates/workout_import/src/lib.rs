//! Workout file parsing and the workout model.
//!
//! A parity-tested port of the Dart prototype's `workout_import` package:
//!
//! - **Formats**: `.zwo` (Zwift), `.erg` and `.mrc` (TrainerRoad/CompuTrainer), dispatched through
//!   one registry ([`WorkoutFormat`], [`parse_file`]).
//! - **Model**: [`WorkoutPlan`] of [`WorkoutSegment`]s with source-unit [`PowerTarget`]s, resolved
//!   to watts only against the FTP current at query time.
//! - **Timeline**: [`WorkoutTimeline`] is the one interpretation of a plan over time.
//! - **JSON**: [`encode_segments`]/[`decode_segments`] own the persisted, versioned envelope.

mod codec;
mod course_text;
mod error;
mod formats;
mod model;
mod numparse;
mod timeline;
mod zwo;

pub use codec::{
    decode_segments, encode_segments, power_from_json, power_to_json, segment_from_json,
    segment_to_json, DecodedSegments, UnreadableReason, CURRENT_VERSION,
};
pub use course_text::{parse_erg_text, parse_mrc_text};
pub use error::{OutOfRange, WorkoutImportError};
pub use formats::{
    extension_of, format_for_file_name, parse_file, supported_extensions,
    supported_extensions_description, ImportedWorkout, WorkoutFormat,
};
pub use model::{
    IntervalRepeat, IntervalRepeatSpec, PowerAtError, PowerTarget, WorkoutPlan, WorkoutSegment,
};
pub use timeline::WorkoutTimeline;
pub use zwo::parse_zwo_text;
