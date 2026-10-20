//! Field-level validation primitives: one implementation and one code per rule.

use std::time::Duration;

use crate::canonical_json::format_number;
use crate::simulation::error::{RaceSimulationError, Result, SimulationValidationCode as C};
use crate::RiderId;

pub(crate) fn check_finite(value: f64, path: &str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(RaceSimulationError::validation(
            C::NotFinite,
            format!("expected a finite number, got {}", format_number(value)),
            path,
        ))
    }
}

pub(crate) fn check_finite_non_negative(value: f64, path: &str) -> Result<f64> {
    check_finite(value, path)?;
    if value < 0.0 {
        return Err(RaceSimulationError::validation(
            C::NegativeValue,
            format!(
                "expected a non-negative number, got {}",
                format_number(value)
            ),
            path,
        ));
    }
    Ok(value)
}

pub(crate) fn check_finite_positive(value: f64, path: &str) -> Result<f64> {
    check_finite(value, path)?;
    if value <= 0.0 {
        return Err(RaceSimulationError::validation(
            C::NotPositive,
            format!("expected a positive number, got {}", format_number(value)),
            path,
        ));
    }
    Ok(value)
}

pub(crate) fn check_positive_int(value: i64, path: &str) -> Result<i64> {
    if value <= 0 {
        return Err(RaceSimulationError::validation(
            C::NotPositive,
            format!("expected a positive integer, got {value}"),
            path,
        ));
    }
    Ok(value)
}

pub(crate) fn check_non_negative_int(value: i64, path: &str) -> Result<i64> {
    if value < 0 {
        return Err(RaceSimulationError::validation(
            C::NegativeValue,
            format!("expected a non-negative integer, got {value}"),
            path,
        ));
    }
    Ok(value)
}

pub(crate) fn check_positive_duration(value: Duration, path: &str) -> Result<Duration> {
    if value.is_zero() {
        return Err(RaceSimulationError::validation(
            C::NotPositive,
            format!(
                "expected a positive duration, got {}",
                format_duration(value)
            ),
            path,
        ));
    }
    Ok(value)
}

/// Fails unless `items` is in strictly ascending `RiderId` order. A duplicate and an
/// out-of-order entry are different mistakes and get different codes. `suffix` is appended to the
/// reported index (`.id` for rider-bearing items, empty for bare IDs).
pub(crate) fn check_canonical<T>(
    items: &[T],
    id: impl Fn(&T) -> &RiderId,
    path: &str,
    suffix: &str,
) -> Result<()> {
    for i in 1..items.len() {
        let (prev, cur) = (id(&items[i - 1]), id(&items[i]));
        match cur.cmp(prev) {
            std::cmp::Ordering::Equal => {
                return Err(RaceSimulationError::validation(
                    C::DuplicateRiderId,
                    format!("rider \"{cur}\" appears more than once"),
                    format!("{path}[{i}]{suffix}"),
                ))
            }
            std::cmp::Ordering::Less => {
                return Err(RaceSimulationError::validation(
                    C::RidersNotCanonical,
                    format!(
                        "riders must be in ascending id order: \"{cur}\" does not follow \"{prev}\""
                    ),
                    format!("{path}[{i}]{suffix}"),
                ))
            }
            std::cmp::Ordering::Greater => {}
        }
    }
    Ok(())
}

/// The duration diagnostic format: `H:MM:SS.ffffff`.
pub(crate) fn format_duration(d: Duration) -> String {
    let m = d.as_micros();
    format!(
        "{}:{:02}:{:02}.{:06}",
        m / 3_600_000_000,
        m / 60_000_000 % 60,
        m / 1_000_000 % 60,
        m % 1_000_000
    )
}
