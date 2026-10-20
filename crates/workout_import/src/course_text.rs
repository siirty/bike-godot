//! The `[COURSE HEADER]`/`[COURSE DATA]` text structure shared by `.erg` and `.mrc`.

use std::time::Duration;

use crate::error::WorkoutImportError;
use crate::model::{PowerTarget, WorkoutPlan, WorkoutSegment};
use crate::numparse::parse_f64;

struct Breakpoint {
    time_micros: i64,
    value: f64,
}

struct CourseText {
    description: Option<String>,
    breakpoints: Vec<Breakpoint>,
}

#[derive(Clone, Copy)]
enum Section {
    None,
    Header,
    Data,
}

/// Parses `.erg` text: course values are absolute watts.
pub fn parse_erg_text(contents: &str) -> Result<WorkoutPlan, WorkoutImportError> {
    Ok(course_plan(
        parse_course_text(contents, "erg")?,
        PowerTarget::AbsoluteWatts,
    ))
}

/// Parses `.mrc` text: course values are percent of FTP (`75` == 75%).
pub fn parse_mrc_text(contents: &str) -> Result<WorkoutPlan, WorkoutImportError> {
    Ok(course_plan(parse_course_text(contents, "mrc")?, |value| {
        PowerTarget::PercentFtp(value / 100.0)
    }))
}

/// Each breakpoint-to-breakpoint span becomes one ramp; a flat run is a ramp with equal ends.
fn course_plan(course: CourseText, to_power: impl Fn(f64) -> PowerTarget) -> WorkoutPlan {
    let segments = course
        .breakpoints
        .windows(2)
        .map(|pair| WorkoutSegment::Ramp {
            duration: Duration::from_micros((pair[1].time_micros - pair[0].time_micros) as u64),
            start_power: to_power(pair[0].value),
            end_power: to_power(pair[1].value),
            cadence: None,
        })
        .collect();
    WorkoutPlan {
        name: None,
        author: None,
        description: course.description,
        segments,
    }
}

fn parse_course_text(contents: &str, format: &str) -> Result<CourseText, WorkoutImportError> {
    let err = |message: String| WorkoutImportError::with_format(message, format);

    let mut section = Section::None;
    let mut saw_data_section = false;
    let mut description = None;
    let mut breakpoints: Vec<Breakpoint> = Vec::new();

    for (i, raw_line) in split_lines(contents).enumerate() {
        let line = trim_text(raw_line);
        if line.is_empty() {
            continue;
        }
        match line.to_uppercase().as_str() {
            "[COURSE HEADER]" => {
                section = Section::Header;
                continue;
            }
            "[END COURSE HEADER]" | "[END COURSE DATA]" => {
                section = Section::None;
                continue;
            }
            "[COURSE DATA]" => {
                section = Section::Data;
                saw_data_section = true;
                continue;
            }
            _ => {}
        }
        match section {
            Section::Header => {
                if let Some(value) = description_value(line) {
                    description = Some(trim_text(value).to_owned());
                }
            }
            Section::Data => {
                let line_number = i + 1;
                let breakpoint = parse_breakpoint(line, line_number).map_err(err)?;
                if let Some(last) = breakpoints.last() {
                    if breakpoint.time_micros < last.time_micros {
                        return Err(err(format!(
                            "time must not decrease within [COURSE DATA]: {} then {} on line \
                             {line_number}",
                            format_duration(last.time_micros),
                            format_duration(breakpoint.time_micros)
                        )));
                    }
                }
                breakpoints.push(breakpoint);
            }
            Section::None => {}
        }
    }

    if !saw_data_section {
        return Err(err("missing required [COURSE DATA] section".to_owned()));
    }
    if breakpoints.len() < 2 {
        return Err(err(format!(
            "[COURSE DATA] must contain at least 2 breakpoints to form a segment, found {}",
            breakpoints.len()
        )));
    }
    Ok(CourseText {
        description,
        breakpoints,
    })
}

/// Splits on `\r\n`, `\r` or `\n`, like the prototype's `RegExp(r'\r\n|\r|\n')`.
fn split_lines(contents: &str) -> impl Iterator<Item = &str> {
    let mut rest = Some(contents);
    std::iter::from_fn(move || {
        let s = rest?;
        match s.find(['\r', '\n']) {
            None => {
                rest = None;
                Some(s)
            }
            Some(pos) => {
                let skip = if s[pos..].starts_with("\r\n") { 2 } else { 1 };
                rest = Some(&s[pos + skip..]);
                Some(&s[..pos])
            }
        }
    })
}

/// `^DESCRIPTION\s*[=\t]\s*(.+)$`, case-insensitive.
fn description_value(line: &str) -> Option<&str> {
    let keyword = "DESCRIPTION";
    let head = line.get(..keyword.len())?;
    if !head.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let after = &line[keyword.len()..];
    // `\s*[=\t]`: whitespace (possibly including the tab separator itself), then `=` or a tab.
    // Backtracking means a run of whitespace ending in a tab satisfies the separator.
    let trimmed = after.trim_start();
    let rest = if let Some(rest) = trimmed.strip_prefix('=') {
        rest
    } else {
        let ws_len = after.len() - trimmed.len();
        let ws = &after[..ws_len];
        let tab = ws.rfind('\t')?;
        &after[tab + 1..]
    };
    let value = rest.trim_start();
    (!value.is_empty()).then_some(value)
}

fn parse_breakpoint(line: &str, line_number: usize) -> Result<Breakpoint, String> {
    let tokens: Vec<&str> = line
        .split(['\t', ' '])
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.len() != 2 {
        return Err(format!(
            "expected \"minute value\" on line {line_number}, found: \"{line}\""
        ));
    }
    let (Some(minutes), Some(value)) = (parse_f64(tokens[0]), parse_f64(tokens[1])) else {
        return Err(format!(
            "non-numeric data on line {line_number}: \"{line}\""
        ));
    };
    let micros = (minutes * 60.0 * 1_000_000.0).round();
    if !micros.is_finite() || micros < 0.0 || micros > i64::MAX as f64 {
        return Err(format!(
            "time on line {line_number} is not a usable non-negative minute value: \"{line}\""
        ));
    }
    Ok(Breakpoint {
        time_micros: micros as i64,
        value,
    })
}

/// Dart's `String.trim` strips Unicode whitespace, as does Rust's `str::trim`; kept as a named
/// function so the intent (match the prototype) is visible at each call site.
fn trim_text(s: &str) -> &str {
    s.trim()
}

/// Renders microseconds like Dart's `Duration.toString()` (`H:MM:SS.ffffff`), so parse errors
/// read the same as the prototype's.
fn format_duration(micros: i64) -> String {
    let sign = if micros < 0 { "-" } else { "" };
    let m = micros.unsigned_abs();
    format!(
        "{sign}{}:{:02}:{:02}.{:06}",
        m / 3_600_000_000,
        m / 60_000_000 % 60,
        m / 1_000_000 % 60,
        m % 1_000_000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_separators() {
        assert_eq!(description_value("DESCRIPTION = Hard"), Some("Hard"));
        assert_eq!(
            description_value("description\tEasy spin"),
            Some("Easy spin")
        );
        assert_eq!(description_value("DESCRIPTION=x"), Some("x"));
        assert_eq!(description_value("DESCRIPTION x"), None);
        assert_eq!(description_value("FTP = 250"), None);
    }

    #[test]
    fn line_splitting_matches_all_newline_styles() {
        let lines: Vec<_> = split_lines("a\r\nb\rc\nd").collect();
        assert_eq!(lines, ["a", "b", "c", "d"]);
    }
}
