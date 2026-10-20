use std::cmp::Ordering;
use std::fmt;

use crate::simulation::error::{RaceSimulationError, Result, SimulationValidationCode, ROOT_PATH};

/// A rider's stable identity inside one simulation. Exact string equality, no normalisation.
///
/// Ordered by UTF-16 code units, so canonical order (and
/// therefore RNG consumption order and snapshot bytes) has no locale and matches across ports.
/// Every rider collection the contracts expose is in this order.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RiderId(String);

impl RiderId {
    /// A rider ID. Empty is rejected: it could match nothing.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(RaceSimulationError::validation(
                SimulationValidationCode::EmptyRiderId,
                "a rider id must not be empty",
                ROOT_PATH,
            ));
        }
        Ok(Self(value))
    }

    /// The identifier, exactly as written to a snapshot.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Ord for RiderId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.encode_utf16().cmp(other.0.encode_utf16())
    }
}

impl PartialOrd for RiderId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for RiderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for RiderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RiderId({})", self.0)
    }
}

/// `ids` in canonical order.
pub fn canonical_rider_ids(ids: impl IntoIterator<Item = RiderId>) -> Vec<RiderId> {
    let mut v: Vec<RiderId> = ids.into_iter().collect();
    v.sort();
    v
}
