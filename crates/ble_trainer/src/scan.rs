use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use trainer_core::ble::{
    ConnectOptions, DiscoveredTrainer, TrainerConnection as BleConnection, TrainerScan,
};

use crate::error::{TrainerFailure, TrainerFailureKind};
use crate::BleTrainer;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// A trainer found by a scan, by name and opaque id. Compare the id; don't parse it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    /// Platform-specific identity, to pass back to [`TrainerScanner::connect`].
    pub id: String,
    /// The advertised name.
    pub name: String,
}

/// What a scan reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvent {
    /// A trainer, reported once.
    Found(DiscoveredDevice),
    /// Scanning could not start (no adapter, permission denied). Nothing more will be found.
    Failed(TrainerFailure),
}

struct Shared {
    discovered: Mutex<HashMap<String, DiscoveredTrainer>>,
    stopped: AtomicBool,
    scan: tokio::sync::Mutex<Option<TrainerScan>>,
}

impl Shared {
    /// Stops the adapter scan, once. Connecting while still scanning is an adapter-contention
    /// failure on BlueZ and WinRT, so a connect awaits this first.
    async fn stop_scanning(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(scan) = self.scan.lock().await.take() {
            if let Err(e) = scan.stop().await {
                log::warn!("scan: stop failed: {e}");
            }
        }
    }
}

/// Scans for FTMS trainers in the background so the rider can pick one.
pub struct TrainerScanner {
    shared: Arc<Shared>,
    events: mpsc::Receiver<ScanEvent>,
}

impl std::fmt::Debug for TrainerScanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrainerScanner").finish_non_exhaustive()
    }
}

impl TrainerScanner {
    /// Starts scanning. Results arrive through [`Self::poll`].
    pub fn start() -> Self {
        let shared = Arc::new(Shared {
            discovered: Mutex::new(HashMap::new()),
            stopped: AtomicBool::new(false),
            scan: tokio::sync::Mutex::new(None),
        });
        let (tx, events) = mpsc::channel();
        let task_shared = shared.clone();
        crate::runtime().spawn(async move {
            let shared = task_shared;
            match TrainerScan::start().await {
                Ok(scan) => *shared.scan.lock().await = Some(scan),
                Err(e) => {
                    let _ = tx.send(ScanEvent::Failed(e.into()));
                    return;
                }
            }
            while !shared.stopped.load(Ordering::SeqCst) {
                let found = {
                    let mut guard = shared.scan.lock().await;
                    let Some(scan) = guard.as_mut() else { break };
                    match scan.poll_new().await {
                        Ok(found) => found,
                        Err(e) => {
                            // One bad poll (a D-Bus hiccup) must not end discovery.
                            log::warn!("scan: poll failed, retrying: {e}");
                            Vec::new()
                        }
                    }
                };
                for trainer in found {
                    let device = DiscoveredDevice {
                        id: trainer.id(),
                        name: trainer.name.clone(),
                    };
                    shared
                        .discovered
                        .lock()
                        .unwrap()
                        .insert(device.id.clone(), trainer);
                    if tx.send(ScanEvent::Found(device)).is_err() {
                        shared.stop_scanning().await;
                        return;
                    }
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
            shared.stop_scanning().await;
        });
        Self { shared, events }
    }

    /// What was found since the last call.
    pub fn poll(&mut self) -> Vec<ScanEvent> {
        self.events.try_iter().collect()
    }

    /// Stops discovering. Devices already found can still be connected to.
    pub fn stop(&self) {
        self.shared.stopped.store(true, Ordering::SeqCst);
    }

    /// A trainer for a device this scan reported. Scanning stops; the trainer connects when its
    /// `connect` is called, after the adapter has actually stopped scanning.
    pub fn connect(&self, device_id: &str) -> Result<BleTrainer, TrainerFailure> {
        let peripheral = {
            let discovered = self.shared.discovered.lock().unwrap();
            let trainer = discovered.get(device_id).ok_or_else(|| {
                TrainerFailure::new(
                    TrainerFailureKind::UnknownDevice,
                    format!(
                        "no discovered device with id {device_id:?} ({} discovered so far)",
                        discovered.len()
                    ),
                )
            })?;
            trainer.peripheral.clone()
        };
        self.stop();
        let shared = self.shared.clone();
        Ok(BleTrainer::with_link(async move {
            shared.stop_scanning().await;
            BleConnection::connect_to_peripheral(peripheral, ConnectOptions::default())
                .await
                .map_err(TrainerFailure::from)
        }))
    }
}

impl Drop for TrainerScanner {
    fn drop(&mut self) {
        self.stop();
    }
}
