use crate::model::{LocalStorageError, SavedWorkoutRecord, WorkoutToSave};
use crate::time::{Clock, SystemClock};
use crate::WorkoutRepository;

/// A repository with no SQLite at all, for UI and adapter tests that only need *a* repository.
/// Tests of storage itself use [`crate::SqliteWorkoutRepository::open_in_memory`].
pub struct InMemoryWorkoutRepository {
    records: Vec<SavedWorkoutRecord>,
    next_id: u64,
    clock: Box<dyn Clock>,
    revision: u64,
}

impl Default for InMemoryWorkoutRepository {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            next_id: 0,
            clock: Box::new(SystemClock),
            revision: 0,
        }
    }
}

impl std::fmt::Debug for InMemoryWorkoutRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryWorkoutRepository")
            .field("records", &self.records)
            .finish_non_exhaustive()
    }
}

impl InMemoryWorkoutRepository {
    /// Empty, stamped by `clock`.
    pub fn with_clock(clock: impl Clock + 'static) -> Self {
        Self {
            clock: Box::new(clock),
            ..Self::default()
        }
    }

    /// Adds a record as given, readable or not.
    pub fn seed(&mut self, record: SavedWorkoutRecord) {
        self.records.push(record);
        self.revision += 1;
    }

    fn sorted(&self) -> Vec<SavedWorkoutRecord> {
        // Stable sort over insertion order, reversed: the same tie-break as SQLite's rowid.
        let mut out: Vec<_> = self.records.iter().rev().cloned().collect();
        out.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        out
    }
}

impl WorkoutRepository for InMemoryWorkoutRepository {
    fn save_workout(&mut self, workout: WorkoutToSave) -> Result<String, LocalStorageError> {
        let id = format!("in-memory-{}", self.next_id);
        self.next_id += 1;
        let now = self.clock.now();
        let plan = workout.plan;
        self.records.push(SavedWorkoutRecord {
            id: id.clone(),
            name: plan.name.clone(),
            author: plan.author.clone(),
            description: plan.description.clone(),
            plan: Ok(plan),
            origin: workout.origin,
            source_format: workout.source_format.tag().to_owned(),
            source_file_name: workout.source_file_name,
            created_at: now,
            updated_at: now,
        });
        self.revision += 1;
        Ok(id)
    }

    fn list_saved_workouts(&self) -> Result<Vec<SavedWorkoutRecord>, LocalStorageError> {
        Ok(self.sorted())
    }

    fn get_saved_workout(&self, id: &str) -> Result<Option<SavedWorkoutRecord>, LocalStorageError> {
        Ok(self.records.iter().find(|r| r.id == id).cloned())
    }

    fn delete_saved_workout(&mut self, id: &str) -> Result<(), LocalStorageError> {
        let before = self.records.len();
        self.records.retain(|r| r.id != id);
        if self.records.len() != before {
            self.revision += 1;
        }
        Ok(())
    }

    fn revision(&self) -> u64 {
        self.revision
    }
}
