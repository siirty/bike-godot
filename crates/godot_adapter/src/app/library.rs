//! The saved-workout library: import, list, detail, delete.

use std::path::Path;

use local_storage::{
    SavedWorkoutRecord, SqliteWorkoutRepository, WorkoutRepository, WorkoutToSave,
};
use workout_import::{parse_file, WorkoutPlan};

use crate::app::format::{
    describe_segment, describe_unreadable, explain_unreadable, format_duration,
    import_error_message,
};

/// One row of the workout list.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkoutSummary {
    /// Repository id.
    pub id: String,
    /// Title, or "Untitled workout".
    pub title: String,
    /// `ZWO · 45:00 · 6 segments`, or `ZWO · Could not be read`.
    pub subtitle: String,
    /// Whether it can be ridden.
    pub readable: bool,
}

/// Everything the detail screen shows.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkoutDetail {
    /// Repository id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Author, if any.
    pub author: Option<String>,
    /// Description, if any.
    pub description: Option<String>,
    /// `45:00 total · 6 segments · ZWO`, or the format and file name of an unreadable one.
    pub summary: String,
    /// One line per segment; empty when unreadable.
    pub segments: Vec<String>,
    /// Why it cannot be read, in full, when it cannot.
    pub unreadable: Option<String>,
}

/// The library over a repository.
pub struct Library {
    repository: Box<dyn WorkoutRepository>,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library").finish_non_exhaustive()
    }
}

fn title(name: Option<&str>) -> String {
    name.unwrap_or("Untitled workout").to_owned()
}

impl Library {
    /// The on-disk library at `path`, created if needed.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let repository = SqliteWorkoutRepository::open(path).map_err(|e| e.to_string())?;
        Ok(Self::with_repository(Box::new(repository)))
    }

    /// Over any repository (tests use the in-memory one).
    pub fn with_repository(repository: Box<dyn WorkoutRepository>) -> Self {
        Self { repository }
    }

    /// Bumps whenever the list changes.
    pub fn revision(&self) -> u64 {
        self.repository.revision()
    }

    /// Newest first, unreadable rows included.
    pub fn list(&self) -> Result<Vec<WorkoutSummary>, String> {
        let records = self
            .repository
            .list_saved_workouts()
            .map_err(|e| e.to_string())?;
        Ok(records.iter().map(summary).collect())
    }

    /// Parses the file at `path` and saves it. The error is already worded for the rider.
    pub fn import_file(&mut self, path: &Path) -> Result<String, String> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "That file has no name this app can read.".to_owned())?;
        let bytes = std::fs::read(path).map_err(|e| format!("Couldn't open that file: {e}."))?;
        let imported = parse_file(file_name, &bytes).map_err(|e| import_error_message(&e))?;
        self.repository
            .save_workout(WorkoutToSave::imported(
                imported.plan,
                imported.format,
                Some(file_name.to_owned()),
            ))
            .map_err(|e| format!("Couldn't save the workout: {e}."))
    }

    /// The detail view of one workout, or `None` for an unknown id.
    pub fn detail(&self, id: &str) -> Result<Option<WorkoutDetail>, String> {
        let record = self
            .repository
            .get_saved_workout(id)
            .map_err(|e| e.to_string())?;
        Ok(record.map(|r| detail(&r)))
    }

    /// The plan to ride, when the workout exists and is readable.
    pub fn plan(&self, id: &str) -> Result<Option<WorkoutPlan>, String> {
        let record = self
            .repository
            .get_saved_workout(id)
            .map_err(|e| e.to_string())?;
        Ok(record.and_then(|r| r.plan.ok()))
    }

    /// Deletes a workout.
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.repository
            .delete_saved_workout(id)
            .map_err(|e| e.to_string())
    }
}

fn summary(r: &SavedWorkoutRecord) -> WorkoutSummary {
    let format = r.source_format.to_uppercase();
    let subtitle = match &r.plan {
        Ok(plan) => format!(
            "{format} · {} · {} segments",
            format_duration(plan.total_duration()),
            plan.segments.len()
        ),
        Err(u) => format!("{format} · {}", describe_unreadable(u.kind)),
    };
    WorkoutSummary {
        id: r.id.clone(),
        title: title(r.name.as_deref()),
        subtitle,
        readable: r.is_readable(),
    }
}

fn detail(r: &SavedWorkoutRecord) -> WorkoutDetail {
    let format = r.source_format.to_uppercase();
    match &r.plan {
        Ok(plan) => WorkoutDetail {
            id: r.id.clone(),
            title: title(plan.name.as_deref()),
            author: plan.author.clone(),
            description: plan.description.clone(),
            summary: format!(
                "{} total · {} segments · {format}",
                format_duration(plan.total_duration()),
                plan.segments.len()
            ),
            segments: plan.segments.iter().map(describe_segment).collect(),
            unreadable: None,
        },
        Err(u) => WorkoutDetail {
            id: r.id.clone(),
            title: title(r.name.as_deref()),
            author: r.author.clone(),
            description: r.description.clone(),
            summary: match &r.source_file_name {
                Some(f) => format!("{format} · {f}"),
                None => format,
            },
            segments: Vec::new(),
            unreadable: Some(format!(
                "{}. {}",
                describe_unreadable(u.kind),
                explain_unreadable(u.kind)
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use local_storage::InMemoryWorkoutRepository;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/workouts")
            .join(name)
    }

    #[test]
    fn imports_lists_and_details_a_real_workout_file() {
        let mut library = Library::with_repository(Box::new(InMemoryWorkoutRepository::default()));
        let id = library.import_file(&fixture("Ramp_Z2_to_Z4.zwo")).unwrap();
        let list = library.list().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].readable);
        assert!(
            list[0].subtitle.starts_with("ZWO · "),
            "{}",
            list[0].subtitle
        );
        let detail = library.detail(&id).unwrap().unwrap();
        assert!(!detail.segments.is_empty());
        assert!(detail.unreadable.is_none());
        assert!(library.plan(&id).unwrap().is_some());
        library.delete(&id).unwrap();
        assert!(library.list().unwrap().is_empty());
    }

    #[test]
    fn an_unsupported_file_is_refused_with_a_message_for_people() {
        let mut library = Library::with_repository(Box::new(InMemoryWorkoutRepository::default()));
        let dir = std::env::temp_dir().join(format!("godot-adapter-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notes.txt");
        std::fs::write(&path, "hello").unwrap();
        let message = library.import_file(&path).unwrap_err();
        assert!(!message.contains("WorkoutImportError"), "{message}");
        assert!(message.ends_with('.'), "{message}");
    }
}
