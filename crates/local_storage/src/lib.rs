//! Local-first persistence for saved workouts.
//!
//! SQLite (bundled through rusqlite) behind the [`WorkoutRepository`] trait. Row types and the
//! connection never leave this crate: callers see [`SavedWorkoutRecord`] only.
//!
//! - Every syncable table has a UUID text primary key, `created_at`/`updated_at`, and nullable
//!   `dirty`/`synced_at` columns. No code path sets the sync columns yet; they exist so a future
//!   sync backend needs no migration.
//! - `segments_json` is owned by `workout_import`: this crate only stores the codec's output.
//! - A row whose segments cannot be decoded is **kept and surfaced as unreadable**, the same way
//!   from every read path, never silently dropped.

mod memory;
mod model;
mod sqlite;
mod time;

pub use memory::InMemoryWorkoutRepository;
pub use model::{
    LocalStorageError, SavedWorkoutRecord, UnreadableWorkout, WorkoutOrigin, WorkoutToSave,
};
pub use sqlite::{SqliteWorkoutRepository, SCHEMA_VERSION};
pub use time::{format_timestamp, parse_timestamp, Clock, SystemClock};

/// Reads and writes saved workouts.
///
/// Synchronous: SQLite on a local file is fast enough to call from a frame, and an engine UI
/// re-lists after a mutation (or polls [`Self::revision`]) instead of subscribing to a stream.
pub trait WorkoutRepository {
    /// Persists a workout and returns its new id.
    fn save_workout(&mut self, workout: WorkoutToSave) -> Result<String, LocalStorageError>;

    /// Every saved workout, newest first. Unreadable rows are included.
    fn list_saved_workouts(&self) -> Result<Vec<SavedWorkoutRecord>, LocalStorageError>;

    /// One saved workout by id, or `None`. Agrees with the list about readability.
    fn get_saved_workout(&self, id: &str) -> Result<Option<SavedWorkoutRecord>, LocalStorageError>;

    /// Deletes a workout. Deleting an unknown id is not an error.
    fn delete_saved_workout(&mut self, id: &str) -> Result<(), LocalStorageError>;

    /// Increments on every successful change, so a view can tell whether to re-list.
    fn revision(&self) -> u64;
}
