---
description: Persistence, serialization and migration conventions for the workout library
paths:
  - "crates/local_storage/**"
---

# local_storage conventions

- **The schema is SQLite via rusqlite**, with hand-written SQL. Every syncable table has:
  - UUID text primary keys;
  - `created_at` / `updated_at`;
  - nullable `dirty` / `synced_at` columns.

  The sync columns exist now so that future cloud sync doesn't need a migration. Today no code path
  sets them to anything but `NULL`. That is expected, not a bug.
- **The schema version is `PRAGMA user_version`** (`SCHEMA_VERSION`). A file at an unknown version
  is refused with `LocalStorageError::NoMigration`, and nothing is created in it. A schema change:
  1. bumps the version;
  2. adds an explicit migration from the previous one;
  3. updates the test that pins the exact column list, nullability and primary key.
- **The `saved_workouts` table's `origin` (`userImport` | `bundled`) is orthogonal to
  `source_format` (`'zwo'|'erg'|'mrc'`).** A bundled sample workout is still a real file of one of
  those formats, parsed through the same `workout_import` pipeline. Only where the bytes came from
  differs. Only `userImport` is written today.
- **`WorkoutPlan.segments` are stored as JSON in one `TEXT` column** (`segments_json`). They are not
  normalized into per-segment rows, because segments are always read and written as a whole plan.
- **`workout_import` owns that JSON, not this crate.** Encode and decode only via
  `workout_import::codec` (`encode_segments` / `decode_segments`). This crate maps the column and
  nothing else. A second interpreter here would re-implement another crate's model invariants
  across a crate boundary, with no compile-time link to the model. The envelope carries a version
  (`{"version": 1, "segments": [...]}`), so a row written by a future build is distinguishable from
  corruption.
- **A row whose `segments_json` cannot be decoded is kept and surfaced as unreadable**, never
  silently dropped: `SavedWorkoutRecord.plan` is `Err(UnreadableWorkout)`. Listing and
  `get_saved_workout` must agree about that row. A workout a user cannot reach from the list must
  not be reachable by another path either.
- **Timestamps are fixed-width UTC text** (`YYYY-MM-DDTHH:MM:SS.ffffffZ`), so text order is time
  order. Ties are broken by insertion order (`rowid`). Do not switch to whole-second integers:
  "newest first" must hold for two workouts saved within the same second.
- **The clock is injectable** (`with_clock`). Tests use it instead of sleeping.
- **Never let rusqlite types leave this crate** (`Row`, `Connection`, statements). Callers see only
  `SavedWorkoutRecord`, `WorkoutToSave` and the `WorkoutRepository` trait. `InMemoryWorkoutRepository`
  implements the same trait, and both implementations run the same behaviour tests.
