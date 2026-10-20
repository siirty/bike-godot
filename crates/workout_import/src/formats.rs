//! The registry of workout file formats: one place that knows which extensions are supported.

use crate::course_text::{parse_erg_text, parse_mrc_text};
use crate::error::WorkoutImportError;
use crate::model::WorkoutPlan;
use crate::zwo::parse_zwo_text;

/// A supported workout file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkoutFormat {
    /// Zwift `.zwo` (XML, percent of FTP).
    Zwo,
    /// TrainerRoad/CompuTrainer `.erg` (text, absolute watts).
    Erg,
    /// TrainerRoad/CompuTrainer `.mrc` (text, percent of FTP).
    Mrc,
}

impl WorkoutFormat {
    /// Every supported format, in registry order.
    pub const ALL: [WorkoutFormat; 3] =
        [WorkoutFormat::Zwo, WorkoutFormat::Erg, WorkoutFormat::Mrc];

    /// The format tag, which is also the lowercase extension and the persisted `source_format`.
    pub fn tag(self) -> &'static str {
        match self {
            WorkoutFormat::Zwo => "zwo",
            WorkoutFormat::Erg => "erg",
            WorkoutFormat::Mrc => "mrc",
        }
    }

    /// Looks a format up by its tag / extension (case-insensitive).
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|f| f.tag().eq_ignore_ascii_case(tag))
    }

    /// Parses a file's raw bytes. Every supported format is UTF-8 text today; taking bytes keeps
    /// room for binary `.fit` without an API break.
    pub fn parse(self, bytes: &[u8]) -> Result<WorkoutPlan, WorkoutImportError> {
        let text = std::str::from_utf8(bytes).map_err(|e| {
            WorkoutImportError::with_format(
                format!("file is not valid UTF-8 text: {e}"),
                self.tag(),
            )
        })?;
        match self {
            WorkoutFormat::Zwo => parse_zwo_text(text),
            WorkoutFormat::Erg => parse_erg_text(text),
            WorkoutFormat::Mrc => parse_mrc_text(text),
        }
    }
}

/// Every supported extension, lowercase and without a dot — suitable for a file dialog filter.
pub fn supported_extensions() -> Vec<&'static str> {
    WorkoutFormat::ALL.iter().map(|f| f.tag()).collect()
}

/// A human-readable list of the supported extensions, e.g. `".zwo, .erg, or .mrc"`.
pub fn supported_extensions_description() -> String {
    let dotted: Vec<String> = supported_extensions()
        .iter()
        .map(|e| format!(".{e}"))
        .collect();
    match dotted.split_last() {
        None => String::new(),
        Some((only, [])) => only.clone(),
        Some((last, rest)) => format!("{}, or {last}", rest.join(", ")),
    }
}

/// The lowercase extension of `file_name` (without the dot), or `None` if it has none.
pub fn extension_of(file_name: &str) -> Option<String> {
    let dot = file_name.rfind('.')?;
    if dot == file_name.len() - 1 {
        return None;
    }
    Some(file_name[dot + 1..].to_lowercase())
}

/// The format for `file_name`'s extension, or an error naming the supported set.
pub fn format_for_file_name(file_name: &str) -> Result<WorkoutFormat, WorkoutImportError> {
    let extension = extension_of(file_name);
    extension
        .as_deref()
        .and_then(WorkoutFormat::from_tag)
        .ok_or_else(|| {
            let shown = extension.clone().unwrap_or_else(|| file_name.to_owned());
            let message = format!(
                "Unsupported file type \"{shown}\" — expected {}.",
                supported_extensions_description()
            );
            match extension {
                Some(ext) => WorkoutImportError::with_format(message, ext),
                None => WorkoutImportError::new(message),
            }
        })
}

/// A successfully parsed workout, plus which format it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedWorkout {
    /// The parsed plan.
    pub plan: WorkoutPlan,
    /// The format it came from.
    pub format: WorkoutFormat,
}

/// Picks the parser for `file_name`, parses `bytes`, and names the file in any failure.
pub fn parse_file(file_name: &str, bytes: &[u8]) -> Result<ImportedWorkout, WorkoutImportError> {
    let format = format_for_file_name(file_name)?;
    let plan = format
        .parse(bytes)
        .map_err(|e| e.reworded(format!("Could not read \"{file_name}\": {}", e.message())))?;
    Ok(ImportedWorkout { plan, format })
}
