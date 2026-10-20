//! The repository contract, against SQLite (in memory and on disk) and the in-memory double.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use local_storage::{
    InMemoryWorkoutRepository, LocalStorageError, SqliteWorkoutRepository, WorkoutOrigin,
    WorkoutRepository, WorkoutToSave, SCHEMA_VERSION,
};
use workout_import::{PowerTarget, UnreadableReason, WorkoutFormat, WorkoutPlan, WorkoutSegment};

fn sample_plan(name: Option<&str>) -> WorkoutPlan {
    WorkoutPlan {
        name: name.map(str::to_owned),
        author: Some("author".into()),
        description: Some("a test plan".into()),
        segments: vec![
            WorkoutSegment::SteadyState {
                duration: Duration::from_secs(300),
                target_power: PowerTarget::PercentFtp(0.75),
                cadence: Some(90),
            },
            WorkoutSegment::Ramp {
                duration: Duration::from_secs(120),
                start_power: PowerTarget::PercentFtp(0.5),
                end_power: PowerTarget::PercentFtp(1.0),
                cadence: None,
            },
        ],
    }
}

/// A clock that ticks 1 µs per read: distinct, ordered stamps without sleeping.
fn ticking_clock() -> impl Fn() -> SystemTime {
    let t = Rc::new(Cell::new(1_790_000_000_000_000u64));
    move || {
        t.set(t.get() + 1);
        UNIX_EPOCH + Duration::from_micros(t.get())
    }
}

fn repositories() -> Vec<(&'static str, Box<dyn WorkoutRepository>)> {
    vec![
        (
            "sqlite",
            Box::new(
                SqliteWorkoutRepository::open_in_memory()
                    .unwrap()
                    .with_clock(ticking_clock()),
            ),
        ),
        (
            "memory",
            Box::new(InMemoryWorkoutRepository::with_clock(ticking_clock())),
        ),
    ]
}

#[test]
fn save_then_get_round_trips_the_plan_and_metadata() {
    for (name, mut repo) in repositories() {
        let plan = sample_plan(Some("Sweet Spot"));
        let id = repo
            .save_workout(WorkoutToSave::imported(
                plan.clone(),
                WorkoutFormat::Zwo,
                Some("sweet_spot.zwo".into()),
            ))
            .unwrap();
        let record = repo.get_saved_workout(&id).unwrap().expect(name);
        assert_eq!(record.id, id);
        assert_eq!(record.source_format, "zwo");
        assert_eq!(record.source_file_name.as_deref(), Some("sweet_spot.zwo"));
        assert_eq!(record.origin, WorkoutOrigin::UserImport);
        assert_eq!(record.plan.as_ref().unwrap(), &plan, "{name}");
        assert_eq!(record.name.as_deref(), Some("Sweet Spot"));
        assert_eq!(record.created_at, record.updated_at, "{name}");
        let total = plan.total_duration();
        for t in [Duration::ZERO, total / 2, total] {
            assert_eq!(
                record.plan.as_ref().unwrap().power_at(t, 200.0),
                plan.power_at(t, 200.0)
            );
        }
    }
}

#[test]
fn unknown_ids_are_none_and_deleting_them_is_harmless() {
    for (name, mut repo) in repositories() {
        assert!(
            repo.get_saved_workout("does-not-exist").unwrap().is_none(),
            "{name}"
        );
        let before = repo.revision();
        repo.delete_saved_workout("does-not-exist").unwrap();
        assert_eq!(repo.revision(), before, "{name}: nothing changed");
    }
}

#[test]
fn list_is_newest_first_and_delete_removes_from_both_paths() {
    for (name, mut repo) in repositories() {
        let first = repo
            .save_workout(WorkoutToSave::imported(
                sample_plan(Some("first")),
                WorkoutFormat::Zwo,
                None,
            ))
            .unwrap();
        let second = repo
            .save_workout(WorkoutToSave::imported(
                sample_plan(Some("second")),
                WorkoutFormat::Erg,
                None,
            ))
            .unwrap();
        let ids: Vec<_> = repo
            .list_saved_workouts()
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec![second.clone(), first.clone()], "{name}");

        let revision = repo.revision();
        repo.delete_saved_workout(&second).unwrap();
        assert!(repo.revision() > revision, "{name}");
        assert!(repo.get_saved_workout(&second).unwrap().is_none());
        assert_eq!(repo.list_saved_workouts().unwrap().len(), 1);
    }
}

#[test]
fn saves_in_the_same_microsecond_still_list_newest_first() {
    let fixed = || UNIX_EPOCH + Duration::from_secs(1_790_000_000);
    let mut sqlite = SqliteWorkoutRepository::open_in_memory()
        .unwrap()
        .with_clock(fixed);
    let mut memory = InMemoryWorkoutRepository::with_clock(fixed);
    for repo in [&mut sqlite as &mut dyn WorkoutRepository, &mut memory] {
        let a = repo
            .save_workout(WorkoutToSave::imported(
                sample_plan(None),
                WorkoutFormat::Mrc,
                None,
            ))
            .unwrap();
        let b = repo
            .save_workout(WorkoutToSave::imported(
                sample_plan(None),
                WorkoutFormat::Mrc,
                None,
            ))
            .unwrap();
        let ids: Vec<_> = repo
            .list_saved_workouts()
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec![b, a]);
    }
}

#[test]
fn ids_are_uuid_v4_and_the_sync_columns_start_null() {
    let mut repo = SqliteWorkoutRepository::open_in_memory().unwrap();
    let id = repo
        .save_workout(WorkoutToSave::imported(
            sample_plan(None),
            WorkoutFormat::Zwo,
            None,
        ))
        .unwrap();
    let parsed = uuid::Uuid::parse_str(&id).unwrap();
    assert_eq!(parsed.get_version_num(), 4);
    assert_eq!(repo.sync_columns(&id).unwrap(), Some((None, None)));
}

fn insert_raw(repo: &mut SqliteWorkoutRepository, id: &str, segments_json: &str) {
    repo.insert_raw_row(
        id,
        Some("Damaged ride"),
        "userImport",
        "zwo",
        segments_json,
        "2026-10-20T08:00:00.000000Z",
    )
    .unwrap();
}

#[test]
fn an_undecodable_row_is_kept_marked_and_agrees_across_read_paths() {
    let mut repo = SqliteWorkoutRepository::open_in_memory().unwrap();
    insert_raw(&mut repo, "bad-json-id", "not valid json");

    let listed = repo.list_saved_workouts().unwrap();
    assert_eq!(listed.len(), 1, "kept, not dropped");
    let from_list = &listed[0];
    assert!(!from_list.is_readable());
    assert_eq!(
        from_list.name.as_deref(),
        Some("Damaged ride"),
        "metadata columns survive"
    );

    let from_get = repo.get_saved_workout("bad-json-id").unwrap().unwrap();
    assert_eq!(&from_get, from_list);
    assert_eq!(from_get.plan.unwrap_err().kind, UnreadableReason::Malformed);
}

#[test]
fn a_row_from_a_newer_build_is_too_new_not_corrupt() {
    let mut repo = SqliteWorkoutRepository::open_in_memory().unwrap();
    insert_raw(
        &mut repo,
        "future-id",
        r#"{"version":9999,"segments":[{"type":"somethingNew"}]}"#,
    );
    insert_raw(
        &mut repo,
        "unknown-type-id",
        r#"{"version":1,"segments":[{"type":"somethingNew"}]}"#,
    );

    let future = repo
        .get_saved_workout("future-id")
        .unwrap()
        .unwrap()
        .plan
        .unwrap_err();
    assert_eq!(
        (future.kind, future.version),
        (UnreadableReason::TooNew, Some(9999))
    );
    let unknown = repo
        .get_saved_workout("unknown-type-id")
        .unwrap()
        .unwrap()
        .plan
        .unwrap_err();
    assert_eq!(unknown.kind, UnreadableReason::Malformed);
}

#[test]
fn an_unknown_origin_fails_every_read_path() {
    let mut repo = SqliteWorkoutRepository::open_in_memory().unwrap();
    repo.insert_raw_row(
        "bad-origin",
        None,
        "notARealOrigin",
        "zwo",
        "[]",
        "2026-10-20T08:00:00.000000Z",
    )
    .unwrap();
    assert!(matches!(
        repo.list_saved_workouts(),
        Err(LocalStorageError::CorruptData(_))
    ));
    assert!(matches!(
        repo.get_saved_workout("bad-origin"),
        Err(LocalStorageError::CorruptData(_))
    ));
}

fn temp_db(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("workouts.sqlite")
}

#[test]
fn a_saved_workout_survives_reopening_the_file() {
    let path = temp_db("reopen");
    let id = {
        let mut repo = SqliteWorkoutRepository::open(&path).unwrap();
        repo.save_workout(WorkoutToSave::imported(
            sample_plan(Some("kept")),
            WorkoutFormat::Zwo,
            None,
        ))
        .unwrap()
    };
    let repo = SqliteWorkoutRepository::open(&path).unwrap();
    assert_eq!(
        repo.get_saved_workout(&id)
            .unwrap()
            .unwrap()
            .name
            .as_deref(),
        Some("kept")
    );
}

#[test]
fn a_fresh_file_gets_exactly_the_expected_schema() {
    let path = temp_db("schema");
    drop(SqliteWorkoutRepository::open(&path).unwrap());
    let c = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = c
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    let mut statement = c.prepare("PRAGMA table_info(saved_workouts)").unwrap();
    // (name, notnull, pk)
    let columns: Vec<(String, bool, bool)> = statement
        .query_map([], |r| {
            Ok((
                r.get(1)?,
                r.get::<_, i64>(3)? == 1,
                r.get::<_, i64>(5)? == 1,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let expected = [
        ("id", true, true),
        ("name", false, false),
        ("author", false, false),
        ("description", false, false),
        ("origin", true, false),
        ("source_format", true, false),
        ("source_file_name", false, false),
        ("segments_json", true, false),
        ("created_at", true, false),
        ("updated_at", true, false),
        ("dirty", false, false),
        ("synced_at", false, false),
    ];
    let expected: Vec<_> = expected
        .iter()
        .map(|(n, nn, pk)| (n.to_string(), *nn, *pk))
        .collect();
    assert_eq!(columns, expected);
}

#[test]
fn a_database_from_another_schema_version_is_refused_untouched() {
    let path = temp_db("future");
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("PRAGMA user_version = 7;")
        .unwrap();
    let err = SqliteWorkoutRepository::open(&path).unwrap_err();
    assert!(
        matches!(err, LocalStorageError::NoMigration { from: 7, to: 1 }),
        "{err}"
    );
    let c = rusqlite::Connection::open(&path).unwrap();
    let tables: i64 = c
        .query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        tables, 0,
        "nothing was created in a database this build does not understand"
    );
}
