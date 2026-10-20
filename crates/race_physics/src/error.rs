use thiserror::Error;

/// Every failure this crate reports.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PhysicsError {
    /// An input was out of its documented domain. `name` is the caller-facing field, such as
    /// `rider.totalMassKg`.
    #[error("invalid argument {name} ({value}): {message}")]
    InvalidArgument {
        /// The caller-facing name of the offending value.
        name: &'static str,
        /// The offending value, rendered.
        value: String,
        /// What the value must be.
        message: &'static str,
    },
    /// A solver found no solution in its bracket, or did not converge.
    #[error("solver: {0}")]
    Solver(String),
    /// A step produced a non-finite quantity from validated inputs. A bug, or physically absurd
    /// (but legal) parameters.
    #[error(
        "longitudinal step produced a non-finite {name} ({value}); the inputs were validated, so \
         this is either a race_physics bug or physically absurd rider parameters"
    )]
    NonFiniteResult {
        /// The field that went non-finite.
        name: &'static str,
        /// Its value.
        value: f64,
    },
}

fn invalid(name: &'static str, value: impl ToString, message: &'static str) -> PhysicsError {
    PhysicsError::InvalidArgument {
        name,
        value: value.to_string(),
        message,
    }
}

pub(crate) fn check_finite(value: f64, name: &'static str) -> Result<f64, PhysicsError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid(name, value, "must be finite"))
    }
}

pub(crate) fn check_finite_non_negative(
    value: f64,
    name: &'static str,
) -> Result<f64, PhysicsError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(invalid(name, value, "must be finite and non-negative"))
    }
}

pub(crate) fn check_finite_positive(value: f64, name: &'static str) -> Result<f64, PhysicsError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid(name, value, "must be finite and positive"))
    }
}

/// `(0, 1]`: 1 is "no reduction"; 0 would be a rider with no air around them at all.
pub(crate) fn check_finite_unit_fraction(
    value: f64,
    name: &'static str,
) -> Result<f64, PhysicsError> {
    if value > 0.0 && value <= 1.0 {
        Ok(value)
    } else {
        Err(invalid(name, value, "must be in (0, 1]"))
    }
}

pub(crate) fn check_positive_duration(
    value: std::time::Duration,
    name: &'static str,
) -> Result<std::time::Duration, PhysicsError> {
    if value.is_zero() {
        Err(invalid(name, format!("{value:?}"), "must be positive"))
    } else {
        Ok(value)
    }
}
