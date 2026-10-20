//! The one error type this crate reports parse and model failures with.

use std::fmt;

/// A workout file (or a persisted workout) could not be turned into a valid model value.
///
/// Mirrors the Dart prototype's `WorkoutImportFormatException`: a human-readable message plus the
/// format tag (`"zwo"`, `"erg"`, `"mrc"`) of the parser that raised it, when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkoutImportError {
    message: String,
    format: Option<String>,
}

impl WorkoutImportError {
    /// Creates an error with no format attribution.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            format: None,
        }
    }

    /// Creates an error attributed to the parser for `format`.
    pub fn with_format(message: impl Into<String>, format: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            format: Some(format.into()),
        }
    }

    /// What went wrong.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Which parser raised this, if known.
    pub fn format(&self) -> Option<&str> {
        self.format.as_deref()
    }

    pub(crate) fn reworded(&self, message: String) -> Self {
        Self {
            message,
            format: self.format.clone(),
        }
    }
}

impl fmt::Display for WorkoutImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.format {
            Some(tag) => write!(f, "WorkoutImportError({tag}): {}", self.message),
            None => write!(f, "WorkoutImportError: {}", self.message),
        }
    }
}

impl std::error::Error for WorkoutImportError {}

/// A query on a [`crate::WorkoutTimeline`] fell outside `[0, total_duration]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{t_micros} µs is outside [0, {total_micros}] µs")]
pub struct OutOfRange {
    /// The queried time, in microseconds.
    pub t_micros: i64,
    /// The timeline's total duration, in microseconds.
    pub total_micros: i64,
}
