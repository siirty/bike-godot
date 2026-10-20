//! Typed failures with a machine-readable code and a field path.

use std::fmt;

/// The path used when a failure is about the object as a whole.
pub const ROOT_PATH: &str = "<root>";

/// Which domain invariant a validation failure violated. One code per rule, not per field.
///
/// The order is part of the contract (numeric identity stays stable), matching the prototype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationValidationCode {
    /// A rider ID was empty.
    EmptyRiderId,
    /// The same rider appeared twice in one collection.
    DuplicateRiderId,
    /// A rider collection was not in ascending `RiderId` order.
    RidersNotCanonical,
    /// A rider ID named a rider that is not configured.
    UnknownRiderId,
    /// A configured rider had no entry.
    MissingRiderId,
    /// A rider named itself as draft target.
    SelfDraftTarget,
    /// A value that must be strictly positive was not.
    NotPositive,
    /// A value that must be non-negative was negative.
    NegativeValue,
    /// A double was NaN or infinite.
    NotFinite,
    /// An integer was outside its range (most often a lateral slot).
    OutOfRange,
    /// Terrain samples were empty, did not start at 0 m, or were not strictly increasing.
    TerrainNotOrdered,
    /// A terrain length preceded the last sample.
    TerrainLengthBeforeLastSample,
    /// The RNG state named an algorithm this build does not implement.
    UnsupportedRandomAlgorithm,
    /// The PCG stream was even.
    InvalidRandomStream,
    /// A step's timestep did not equal the configured fixed step.
    TimestepMismatch,
    /// Retired (the kernel steps any positive rider count). Kept for stable numbering.
    RiderCountNotSupported,
    /// A drafting multiplier list was empty.
    DraftingSlotsEmpty,
    /// A drafting multiplier was not finite in `(0, 1]`.
    DraftingMultiplierOutOfRange,
    /// A drafting multiplier list decreased with lateral separation.
    DraftingMultipliersNotMonotonic,
    /// The same-slot drafting multiplier was `>= 1`.
    DraftingSameSlotNotSheltering,
}

impl SimulationValidationCode {
    /// The prototype's enum name, such as `negativeValue`.
    pub fn name(self) -> &'static str {
        use SimulationValidationCode::*;
        match self {
            EmptyRiderId => "emptyRiderId",
            DuplicateRiderId => "duplicateRiderId",
            RidersNotCanonical => "ridersNotCanonical",
            UnknownRiderId => "unknownRiderId",
            MissingRiderId => "missingRiderId",
            SelfDraftTarget => "selfDraftTarget",
            NotPositive => "notPositive",
            NegativeValue => "negativeValue",
            NotFinite => "notFinite",
            OutOfRange => "outOfRange",
            TerrainNotOrdered => "terrainNotOrdered",
            TerrainLengthBeforeLastSample => "terrainLengthBeforeLastSample",
            UnsupportedRandomAlgorithm => "unsupportedRandomAlgorithm",
            InvalidRandomStream => "invalidRandomStream",
            TimestepMismatch => "timestepMismatch",
            RiderCountNotSupported => "riderCountNotSupported",
            DraftingSlotsEmpty => "draftingSlotsEmpty",
            DraftingMultiplierOutOfRange => "draftingMultiplierOutOfRange",
            DraftingMultipliersNotMonotonic => "draftingMultipliersNotMonotonic",
            DraftingSameSlotNotSheltering => "draftingSameSlotNotSheltering",
        }
    }
}

/// Which wire-level problem made bytes unreadable as a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapshotDecodeCode {
    /// Not valid JSON (or a structurally contradictory block).
    MalformedJson,
    /// A required field was absent.
    MissingField,
    /// A field had the wrong JSON type.
    WrongType,
    /// The `format` identifier was foreign.
    UnsupportedFormat,
    /// The schema version is not the one this build reads. Never reinterpreted.
    UnsupportedSchemaVersion,
    /// An enum string this build does not know.
    UnknownEnumValue,
}

impl SnapshotDecodeCode {
    /// The prototype's enum name.
    pub fn name(self) -> &'static str {
        use SnapshotDecodeCode::*;
        match self {
            MalformedJson => "malformedJson",
            MissingField => "missingField",
            WrongType => "wrongType",
            UnsupportedFormat => "unsupportedFormat",
            UnsupportedSchemaVersion => "unsupportedSchemaVersion",
            UnknownEnumValue => "unknownEnumValue",
        }
    }
}

/// Every contract failure: either "these values are not a legal simulation" (a bug in whatever
/// produced them) or "these bytes are not a snapshot this build reads" (a compatibility problem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaceSimulationError {
    /// A domain invariant was violated.
    Validation {
        /// Which rule.
        code: SimulationValidationCode,
        /// Human-readable; not a stable API.
        message: String,
        /// Dotted/indexed path to the field, such as `state.riders[2].speedMs`.
        path: String,
    },
    /// Snapshot bytes were unreadable.
    Decode {
        /// Which wire problem.
        code: SnapshotDecodeCode,
        /// Human-readable; not a stable API.
        message: String,
        /// Path to the field.
        path: String,
    },
}

impl RaceSimulationError {
    pub(crate) fn validation(
        code: SimulationValidationCode,
        message: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self::Validation {
            code,
            message: message.into(),
            path: path.into(),
        }
    }

    pub(crate) fn decode(
        code: SnapshotDecodeCode,
        message: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self::Decode {
            code,
            message: message.into(),
            path: path.into(),
        }
    }

    /// The field path.
    pub fn path(&self) -> &str {
        match self {
            Self::Validation { path, .. } | Self::Decode { path, .. } => path,
        }
    }

    /// The message.
    pub fn message(&self) -> &str {
        match self {
            Self::Validation { message, .. } | Self::Decode { message, .. } => message,
        }
    }

    /// The validation code, if this is a validation failure.
    pub fn validation_code(&self) -> Option<SimulationValidationCode> {
        match self {
            Self::Validation { code, .. } => Some(*code),
            Self::Decode { .. } => None,
        }
    }

    /// The decode code, if this is a decode failure.
    pub fn decode_code(&self) -> Option<SnapshotDecodeCode> {
        match self {
            Self::Decode { code, .. } => Some(*code),
            Self::Validation { .. } => None,
        }
    }

    /// Re-roots a validation failure under `prefix`; decode failures already carry full paths.
    pub(crate) fn prefixed(self, prefix: &str) -> Self {
        match self {
            Self::Validation {
                code,
                message,
                path,
            } => Self::Validation {
                code,
                message,
                path: if path == ROOT_PATH {
                    prefix.to_owned()
                } else {
                    format!("{prefix}.{path}")
                },
            },
            decode @ Self::Decode { .. } => decode,
        }
    }
}

impl fmt::Display for RaceSimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation {
                code,
                message,
                path,
            } => write!(
                f,
                "SimulationValidationException({} at {path}): {message}",
                code.name()
            ),
            Self::Decode {
                code,
                message,
                path,
            } => write!(
                f,
                "SnapshotDecodeException({} at {path}): {message}",
                code.name()
            ),
        }
    }
}

impl std::error::Error for RaceSimulationError {}

/// The crate's result type.
pub type Result<T, E = RaceSimulationError> = std::result::Result<T, E>;

/// Runs `body`, re-rooting any validation failure under `prefix`.
pub(crate) fn at<T>(prefix: &str, body: impl FnOnce() -> Result<T>) -> Result<T> {
    body().map_err(|e| e.prefixed(prefix))
}
