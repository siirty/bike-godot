use std::time::SystemTime;

use thiserror::Error;
use workout_import::{UnreadableReason, WorkoutFormat, WorkoutPlan};

/// Where a saved workout came from. Orthogonal to its file format: a bundled sample is still a
/// real `.zwo`/`.erg`/`.mrc` parsed by the same pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkoutOrigin {
    /// The rider imported a file. The only value written today.
    #[default]
    UserImport,
    /// Shipped with the app. Reserved so a sample library needs no migration.
    Bundled,
}

impl WorkoutOrigin {
    /// The stored name.
    pub fn name(self) -> &'static str {
        match self {
            WorkoutOrigin::UserImport => "userImport",
            WorkoutOrigin::Bundled => "bundled",
        }
    }

    /// The origin a stored name means.
    pub fn by_name(name: &str) -> Option<Self> {
        [WorkoutOrigin::UserImport, WorkoutOrigin::Bundled]
            .into_iter()
            .find(|o| o.name() == name)
    }
}

/// What to save.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkoutToSave {
    /// The parsed plan; its name, author and description become columns.
    pub plan: WorkoutPlan,
    /// The parser used.
    pub source_format: WorkoutFormat,
    /// The original file name, for display only.
    pub source_file_name: Option<String>,
    /// Where it came from.
    pub origin: WorkoutOrigin,
}

impl WorkoutToSave {
    /// A user import of `plan`.
    pub fn imported(
        plan: WorkoutPlan,
        source_format: WorkoutFormat,
        file_name: Option<String>,
    ) -> Self {
        Self {
            plan,
            source_format,
            source_file_name: file_name,
            origin: WorkoutOrigin::UserImport,
        }
    }
}

/// Why a row's segments could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableWorkout {
    /// Damaged, or written by a newer build. Only damage is worth offering to delete.
    pub kind: UnreadableReason,
    /// The stored format version, if one could be read.
    pub version: Option<i64>,
    /// A diagnostic, not for showing to a rider verbatim.
    pub reason: String,
}

/// A saved workout: its metadata, plus its plan when the plan could be read.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedWorkoutRecord {
    /// UUID v4.
    pub id: String,
    /// The plan, or the reason it could not be read. Never both, never neither.
    pub plan: Result<WorkoutPlan, UnreadableWorkout>,
    /// Title, from its own column: available even when the plan is not.
    pub name: Option<String>,
    /// Author.
    pub author: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Where it came from.
    pub origin: WorkoutOrigin,
    /// The stored parser tag (`zwo`, `erg`, `mrc`). Kept as text so a row from a newer build with
    /// a format this one lacks still lists.
    pub source_format: String,
    /// Original file name.
    pub source_file_name: Option<String>,
    /// When it was saved.
    pub created_at: SystemTime,
    /// When it last changed.
    pub updated_at: SystemTime,
}

impl SavedWorkoutRecord {
    /// Whether the plan is available.
    pub fn is_readable(&self) -> bool {
        self.plan.is_ok()
    }
}

/// A storage failure.
#[derive(Debug, Error)]
pub enum LocalStorageError {
    /// SQLite itself failed.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A row cannot be classified at all (an unknown origin, an unparseable timestamp). Unlike an
    /// unreadable segment blob, this is a schema-level problem, so it fails the read.
    #[error("corrupt stored data: {0}")]
    CorruptData(String),
    /// The database's schema version has no migration path to this build's.
    #[error(
        "no migration is defined from schema v{from} to v{to}; add one, and a test for it, in \
         crates/local_storage"
    )]
    NoMigration {
        /// The version on disk.
        from: i64,
        /// The version this build reads.
        to: i64,
    },
}
