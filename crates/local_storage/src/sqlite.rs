use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};
use workout_import::{decode_segments, encode_segments, DecodedSegments, WorkoutPlan};

use crate::model::{
    LocalStorageError, SavedWorkoutRecord, UnreadableWorkout, WorkoutOrigin, WorkoutToSave,
};
use crate::time::{format_timestamp, parse_timestamp, Clock, SystemClock};
use crate::WorkoutRepository;

/// The schema version this build creates and reads (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i64 = 1;

const CREATE_V1: &str = "
CREATE TABLE saved_workouts (
  id               TEXT    NOT NULL PRIMARY KEY,
  name             TEXT,
  author           TEXT,
  description      TEXT,
  origin           TEXT    NOT NULL,
  source_format    TEXT    NOT NULL,
  source_file_name TEXT,
  segments_json    TEXT    NOT NULL,
  created_at       TEXT    NOT NULL,
  updated_at       TEXT    NOT NULL,
  dirty            INTEGER,
  synced_at        TEXT
);
";

const COLUMNS: &str = "id, name, author, description, origin, source_format, source_file_name, \
                       segments_json, created_at, updated_at";

/// A row's `(dirty, synced_at)`.
pub type SyncColumns = (Option<bool>, Option<String>);

/// The real repository: SQLite through rusqlite. The connection stays private.
pub struct SqliteWorkoutRepository {
    connection: Connection,
    clock: Box<dyn Clock>,
    revision: u64,
}

impl std::fmt::Debug for SqliteWorkoutRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteWorkoutRepository")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl SqliteWorkoutRepository {
    /// Opens (creating if needed) the database file at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LocalStorageError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// A private, empty database in memory: the same SQL and migrations, nothing on disk.
    pub fn open_in_memory() -> Result<Self, LocalStorageError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// Replaces the clock that stamps `created_at`/`updated_at`.
    pub fn with_clock(mut self, clock: impl Clock + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    fn from_connection(connection: Connection) -> Result<Self, LocalStorageError> {
        migrate(&connection)?;
        Ok(Self {
            connection,
            clock: Box::new(SystemClock),
            revision: 0,
        })
    }

    /// Inserts a row exactly as given, bypassing every check. The only way to produce a row this
    /// crate considers damaged; for tests of the unreadable-row path, nothing else.
    #[doc(hidden)]
    pub fn insert_raw_row(
        &mut self,
        id: &str,
        name: Option<&str>,
        origin: &str,
        source_format: &str,
        segments_json: &str,
        created_at: &str,
    ) -> Result<(), LocalStorageError> {
        self.connection.execute(
            &format!("INSERT INTO saved_workouts ({COLUMNS}) VALUES (?1, ?2, NULL, NULL, ?3, ?4, NULL, ?5, ?6, ?6)"),
            params![id, name, origin, source_format, segments_json, created_at],
        )?;
        self.revision += 1;
        Ok(())
    }

    /// The sync columns of a row, for tests: `(dirty, synced_at)`.
    #[doc(hidden)]
    pub fn sync_columns(&self, id: &str) -> Result<Option<SyncColumns>, LocalStorageError> {
        Ok(self
            .connection
            .query_row(
                "SELECT dirty, synced_at FROM saved_workouts WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }
}

fn migrate(connection: &Connection) -> Result<(), LocalStorageError> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    match version {
        0 => {
            // A fresh file. Created in one transaction so a crash cannot leave a half schema
            // labelled as version 1.
            connection.execute_batch(&format!(
                "BEGIN; {CREATE_V1} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;"
            ))?;
            Ok(())
        }
        SCHEMA_VERSION => Ok(()),
        // Neither an upgrade nor a database from a newer build is guessed at.
        from => Err(LocalStorageError::NoMigration {
            from,
            to: SCHEMA_VERSION,
        }),
    }
}

struct RawRow {
    id: String,
    name: Option<String>,
    author: Option<String>,
    description: Option<String>,
    origin: String,
    source_format: String,
    source_file_name: Option<String>,
    segments_json: String,
    created_at: String,
    updated_at: String,
}

fn raw_row(r: &Row<'_>) -> rusqlite::Result<RawRow> {
    Ok(RawRow {
        id: r.get(0)?,
        name: r.get(1)?,
        author: r.get(2)?,
        description: r.get(3)?,
        origin: r.get(4)?,
        source_format: r.get(5)?,
        source_file_name: r.get(6)?,
        segments_json: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
    })
}

/// Column mapping is this crate's whole serialization job; the blob is `workout_import`'s.
fn to_record(row: RawRow) -> Result<SavedWorkoutRecord, LocalStorageError> {
    let origin = WorkoutOrigin::by_name(&row.origin).ok_or_else(|| {
        LocalStorageError::CorruptData(format!("unknown WorkoutOrigin: {}", row.origin))
    })?;
    let time = |text: &str, column: &str| {
        parse_timestamp(text).ok_or_else(|| {
            LocalStorageError::CorruptData(format!(
                "unreadable {column} {text:?} on workout {}",
                row.id
            ))
        })
    };
    let created_at = time(&row.created_at, "created_at")?;
    let updated_at = time(&row.updated_at, "updated_at")?;
    let plan = match decode_segments(&row.segments_json) {
        DecodedSegments::Readable { segments, .. } => Ok(WorkoutPlan {
            name: row.name.clone(),
            author: row.author.clone(),
            description: row.description.clone(),
            segments,
        }),
        DecodedSegments::Unreadable {
            reason,
            kind,
            version,
        } => Err(UnreadableWorkout {
            kind,
            version,
            reason,
        }),
    };
    Ok(SavedWorkoutRecord {
        id: row.id,
        plan,
        name: row.name,
        author: row.author,
        description: row.description,
        origin,
        source_format: row.source_format,
        source_file_name: row.source_file_name,
        created_at,
        updated_at,
    })
}

impl WorkoutRepository for SqliteWorkoutRepository {
    fn save_workout(&mut self, workout: WorkoutToSave) -> Result<String, LocalStorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = format_timestamp(self.clock.now());
        let plan = &workout.plan;
        self.connection.execute(
            &format!("INSERT INTO saved_workouts ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)"),
            params![
                id,
                plan.name,
                plan.author,
                plan.description,
                workout.origin.name(),
                workout.source_format.tag(),
                workout.source_file_name,
                encode_segments(&plan.segments),
                now,
            ],
        )?;
        self.revision += 1;
        Ok(id)
    }

    fn list_saved_workouts(&self) -> Result<Vec<SavedWorkoutRecord>, LocalStorageError> {
        // rowid breaks a created_at tie by insertion order, so "newest first" holds even for two
        // saves stamped with the same microsecond.
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM saved_workouts ORDER BY created_at DESC, rowid DESC"
        ))?;
        let rows = statement
            .query_map([], raw_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter().map(to_record).collect()
    }

    fn get_saved_workout(&self, id: &str) -> Result<Option<SavedWorkoutRecord>, LocalStorageError> {
        let row = self
            .connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM saved_workouts WHERE id = ?1"),
                [id],
                raw_row,
            )
            .optional()?;
        row.map(to_record).transpose()
    }

    fn delete_saved_workout(&mut self, id: &str) -> Result<(), LocalStorageError> {
        if self
            .connection
            .execute("DELETE FROM saved_workouts WHERE id = ?1", [id])?
            > 0
        {
            self.revision += 1;
        }
        Ok(())
    }

    fn revision(&self) -> u64 {
        self.revision
    }
}
