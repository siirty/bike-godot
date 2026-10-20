//! `TrainerControl` implementations over `trainer_core`, the separate Rust trainer repository.
//!
//! **This crate is the SIM-only chokepoint.** `trainer_core` is capability-complete and also
//! implements ERG (`set_target_power`), resistance, wheel-circumference and reset writes for its
//! own reusability. None of them is called here, and none is reachable through this crate's public
//! surface; `tests/sim_only_surface.rs` fails the build if one appears in `src/`. The app writes
//! grade, wind, Crr and CdA, and nothing else.
//!
//! - [`CoreSimulatedTrainer`]: the core's momentum simulator, driven in-process by explicit time.
//!   It is the "ride without a trainer" path, and it has the SIM debug surface.
//! - [`BleTrainer`]: a real FTMS trainer. Bluetooth is asynchronous, so a background tokio worker
//!   owns the connection and [`trainer_control::TrainerControl::poll`] drains what it reported.
//! - [`TrainerScanner`]: discovery, so the rider can pick a trainer rather than take the first.

mod ble;
mod convert;
mod error;
mod scan;
mod simulated;

pub use ble::BleTrainer;
pub use error::{TrainerFailure, TrainerFailureKind};
pub use scan::{DiscoveredDevice, ScanEvent, TrainerScanner};
pub use simulated::{CoreSimulatedTrainer, SimDebugSnapshot, SIM_REPORT_INTERVAL, SIM_TICK};

use std::sync::{Arc, OnceLock};

/// The one tokio runtime every Bluetooth worker shares. Two worker threads: BLE is I/O-bound,
/// and nothing here should compete with the engine's own threads.
pub(crate) fn runtime() -> Arc<tokio::runtime::Runtime> {
    static RUNTIME: OnceLock<Arc<tokio::runtime::Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_name("ble-trainer")
                    .enable_all()
                    .build()
                    .expect("a tokio runtime can be built"),
            )
        })
        .clone()
}
