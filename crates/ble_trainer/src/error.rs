use thiserror::Error;
use trainer_core::ble::BleError;

/// What went wrong, coarsely enough for a UI to branch on ("no Bluetooth" vs "no trainer found").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainerFailureKind {
    /// No Bluetooth adapter on this machine.
    NoAdapter,
    /// Scanning found no trainer in time.
    ScanTimedOut,
    /// Connecting, discovering characteristics, or the FTMS handshake failed.
    ConnectFailed,
    /// The trainer cannot do SIM mode.
    CapabilityMismatch,
    /// A write was not acknowledged, or was refused.
    WriteFailed,
    /// The link dropped.
    ConnectionLost,
    /// A discovered-device id that this scan never reported.
    UnknownDevice,
    /// Anything else.
    Other,
}

/// A trainer failure: a kind to branch on and the full message for logs.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct TrainerFailure {
    /// The category.
    pub kind: TrainerFailureKind,
    /// The diagnostic text.
    pub message: String,
}

impl TrainerFailure {
    pub(crate) fn new(kind: TrainerFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<BleError> for TrainerFailure {
    fn from(err: BleError) -> Self {
        use TrainerFailureKind as K;
        let kind = match &err {
            BleError::NoAdapterAvailable => K::NoAdapter,
            BleError::ScanTimedOut { .. } => K::ScanTimedOut,
            BleError::ConnectFailed { .. }
            | BleError::CharacteristicNotFound { .. }
            | BleError::HandshakeFailed(_) => K::ConnectFailed,
            BleError::CapabilityMismatch { .. } => K::CapabilityMismatch,
            BleError::UnexpectedResponse { .. }
            | BleError::AckOpcodeMismatch { .. }
            | BleError::AckTimedOut { .. } => K::WriteFailed,
            BleError::MalformedNotification { .. }
            | BleError::NotificationStreamEnded { .. }
            | BleError::ActorStopped { .. } => K::ConnectionLost,
            BleError::Btleplug(_) => K::Other,
        };
        Self::new(kind, err.to_string())
    }
}

impl From<trainer_core::TrainerError> for TrainerFailure {
    fn from(err: trainer_core::TrainerError) -> Self {
        match err {
            trainer_core::TrainerError::Ble(e) => e.into(),
            other => Self::new(TrainerFailureKind::Other, other.to_string()),
        }
    }
}

impl From<TrainerFailure> for trainer_control::TrainerError {
    fn from(f: TrainerFailure) -> Self {
        trainer_control::TrainerError::new(f.message)
    }
}
