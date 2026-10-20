//! Presentation text for workouts and user-visible errors.
//! Exception types are for logs; these strings are what a rider sees.

use std::time::Duration;

use workout_import::{PowerTarget, UnreadableReason, WorkoutImportError, WorkoutSegment};

/// Shown where a value the trainer does not report would otherwise be blank.
pub const EM_DASH: &str = "—";

/// `M:SS`, or `H:MM:SS` from an hour.
pub fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// km/h with one decimal.
pub fn format_speed(speed_ms: f64) -> String {
    format!("{:.1} km/h", speed_ms * 3.6)
}

/// Whole watts.
pub fn format_power(watts: f64) -> String {
    format!("{} W", round_whole(watts))
}

/// Whole rpm, or an em dash when not reported.
pub fn format_cadence(rpm: Option<f64>) -> String {
    rpm.map_or_else(|| EM_DASH.to_owned(), |r| format!("{} rpm", round_whole(r)))
}

/// Round halves away from zero.
fn round_whole(x: f64) -> i64 {
    x.round() as i64
}

/// A target in its file's own unit.
pub fn format_power_target(target: PowerTarget) -> String {
    match target {
        PowerTarget::PercentFtp(f) => format!("{}% FTP", round_whole(f * 100.0)),
        PowerTarget::AbsoluteWatts(w) => format_power(w),
    }
}

/// One label for a start→end pair, collapsed when both ends read the same.
fn power_range(start: PowerTarget, end: PowerTarget) -> String {
    let (a, b) = (format_power_target(start), format_power_target(end));
    if a == b {
        a
    } else {
        format!("{a} → {b}")
    }
}

/// `20:00 steady @ 75% FTP`, `4 × (3:00 @ 100% FTP / 2:00 @ 50% FTP)`, …
pub fn describe_segment(segment: &WorkoutSegment) -> String {
    match segment {
        WorkoutSegment::SteadyState {
            duration,
            target_power,
            ..
        } => {
            format!(
                "{} steady @ {}",
                format_duration(*duration),
                format_power_target(*target_power)
            )
        }
        WorkoutSegment::Ramp {
            duration,
            start_power,
            end_power,
            ..
        } => {
            format!(
                "{} ramp {}",
                format_duration(*duration),
                power_range(*start_power, *end_power)
            )
        }
        WorkoutSegment::FreeRide { duration, .. } => {
            format!("{} free ride", format_duration(*duration))
        }
        WorkoutSegment::IntervalRepeat(b) => {
            let on = power_range(b.on_start_power(), b.on_end_power());
            match (
                b.off_duration().is_zero(),
                b.off_start_power(),
                b.off_end_power(),
            ) {
                (false, Some(s), Some(e)) => format!(
                    "{} × ({} @ {on} / {} @ {})",
                    b.repeat(),
                    format_duration(b.on_duration()),
                    format_duration(b.off_duration()),
                    power_range(s, e)
                ),
                _ => format!(
                    "{} × {} @ {on}",
                    b.repeat(),
                    format_duration(b.on_duration())
                ),
            }
        }
    }
}

/// A short label for a workout that cannot be read.
pub fn describe_unreadable(kind: UnreadableReason) -> &'static str {
    match kind {
        UnreadableReason::TooNew => "Needs a newer version of the app",
        UnreadableReason::Malformed => "Could not be read",
    }
}

/// The full explanation, for the detail view.
pub fn explain_unreadable(kind: UnreadableReason) -> &'static str {
    match kind {
        UnreadableReason::TooNew => {
            "This workout was saved by a newer version of the app, so its segments can't be read \
             here. Updating the app should restore it — it has not been deleted."
        }
        UnreadableReason::Malformed => {
            "This workout's saved segments are damaged and can't be read. Re-importing the \
             original file will replace it."
        }
    }
}

/// What to tell a rider about a file that would not import. The parser's message is written for
/// a person; the error's type and format tag are not.
pub fn import_error_message(error: &WorkoutImportError) -> String {
    let message = error.message().trim().to_owned();
    if message.is_empty() {
        "That workout file couldn't be read.".to_owned()
    } else if message.ends_with('.') {
        message
    } else {
        format!("{message}.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use workout_import::{IntervalRepeat, IntervalRepeatSpec};

    const fn s(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn durations_speeds_and_powers_read_like_the_prototype() {
        assert_eq!(format_duration(s(0)), "0:00");
        assert_eq!(format_duration(s(65)), "1:05");
        assert_eq!(format_duration(s(3600 + 61)), "1:01:01");
        assert_eq!(format_speed(10.0), "36.0 km/h");
        assert_eq!(format_power(249.5), "250 W");
        assert_eq!(format_cadence(None), "—");
        assert_eq!(format_cadence(Some(89.6)), "90 rpm");
    }

    #[test]
    fn segments_describe_in_their_own_units() {
        let steady = WorkoutSegment::SteadyState {
            duration: s(1200),
            target_power: PowerTarget::PercentFtp(0.75),
            cadence: None,
        };
        assert_eq!(describe_segment(&steady), "20:00 steady @ 75% FTP");
        let ramp = WorkoutSegment::Ramp {
            duration: s(120),
            start_power: PowerTarget::PercentFtp(0.749),
            end_power: PowerTarget::PercentFtp(0.751),
            cadence: None,
        };
        assert_eq!(
            describe_segment(&ramp),
            "2:00 ramp 75% FTP",
            "collapses equal labels"
        );
        let ramp = WorkoutSegment::Ramp {
            duration: s(120),
            start_power: PowerTarget::AbsoluteWatts(100.0),
            end_power: PowerTarget::AbsoluteWatts(200.0),
            cadence: None,
        };
        assert_eq!(describe_segment(&ramp), "2:00 ramp 100 W → 200 W");
        let block = IntervalRepeat::new(IntervalRepeatSpec {
            repeat: 4,
            on_duration: s(180),
            on_start_power: PowerTarget::PercentFtp(1.0),
            on_end_power: PowerTarget::PercentFtp(1.0),
            off_duration: s(120),
            off_start_power: Some(PowerTarget::PercentFtp(0.5)),
            off_end_power: Some(PowerTarget::PercentFtp(0.5)),
            on_cadence: None,
            off_cadence: None,
        })
        .unwrap();
        assert_eq!(
            describe_segment(&WorkoutSegment::IntervalRepeat(block)),
            "4 × (3:00 @ 100% FTP / 2:00 @ 50% FTP)"
        );
        assert_eq!(
            describe_segment(&WorkoutSegment::FreeRide {
                duration: s(300),
                cadence: None
            }),
            "5:00 free ride"
        );
    }

    #[test]
    fn import_errors_show_the_parser_message_not_the_type() {
        let e = WorkoutImportError::with_format("missing required <name> element", "zwo");
        assert_eq!(import_error_message(&e), "missing required <name> element.");
        assert_eq!(
            import_error_message(&WorkoutImportError::new("  ")),
            "That workout file couldn't be read."
        );
    }
}
