use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use tokio::sync::mpsc as tokio_mpsc;
use trainer_control::{
    SimulationParameters, TrainerConnectionState, TrainerControl, TrainerError, TrainerEvent,
};
use trainer_core::ble::{ConnectOptions, TrainerConnection as BleConnection};
use trainer_core::TrainerConnection;

use crate::convert;
use crate::error::{TrainerFailure, TrainerFailureKind};

/// The only writes the app ever sends a trainer. ERG has no variant here, so no caller of this
/// crate can ask for one.
#[derive(Debug)]
enum Command {
    SetSimulationParameters(SimulationParameters),
    StartOrResume,
    Pause,
    Disconnect,
}

#[derive(Debug)]
enum WorkerEvent {
    State(TrainerConnectionState),
    Telemetry(trainer_control::TrainerTelemetry),
    Failure(TrainerFailure),
}

type Spawn =
    Box<dyn FnOnce(tokio_mpsc::UnboundedReceiver<Command>, mpsc::Sender<WorkerEvent>) + Send>;

/// A real FTMS trainer over Bluetooth, in SIM mode.
///
/// A background worker on a shared tokio runtime owns the connection: it connects, applies
/// writes in order, and forwards telemetry. The engine thread never blocks on Bluetooth.
/// [`TrainerControl::connect`] only *starts* connecting; `Connected` (or a failure, see
/// [`Self::take_failures`]) arrives through [`TrainerControl::poll`]. Writes issued before then
/// are queued and applied in order once connected. A trainer connects once; reconnecting is a
/// new `BleTrainer`.
pub struct BleTrainer {
    spawn: Option<Spawn>,
    commands: Option<tokio_mpsc::UnboundedSender<Command>>,
    events_tx: mpsc::Sender<WorkerEvent>,
    events: mpsc::Receiver<WorkerEvent>,
    state: TrainerConnectionState,
    pending: Vec<TrainerEvent>,
    failures: Vec<TrainerFailure>,
}

impl std::fmt::Debug for BleTrainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BleTrainer")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl BleTrainer {
    /// Connects, when asked, to the first FTMS trainer found.
    pub fn first_found() -> Self {
        Self::with_link(async {
            BleConnection::connect(|_| true, ConnectOptions::default())
                .await
                .map_err(TrainerFailure::from)
        })
    }

    /// Connects, when asked, through `connect`. Production code passes a Bluetooth connection;
    /// tests pass a fake link implementing the core's trait. The worker only ever calls its
    /// SIM-safe methods.
    #[doc(hidden)]
    pub fn with_link<C, F>(connect: F) -> Self
    where
        C: TrainerConnection + Clone,
        F: Future<Output = Result<C, TrainerFailure>> + Send + 'static,
    {
        let (events_tx, events) = mpsc::channel();
        let connect: Pin<Box<dyn Future<Output = Result<C, TrainerFailure>> + Send>> =
            Box::pin(connect);
        Self {
            spawn: Some(Box::new(move |commands, events| {
                crate::runtime().spawn(run_worker(connect, commands, events));
            })),
            commands: None,
            events_tx,
            events,
            state: TrainerConnectionState::Disconnected,
            pending: Vec::new(),
            failures: Vec::new(),
        }
    }

    /// Failures reported since the last call: a refused connection, an unacknowledged write, a
    /// dropped link. The ride keeps going; the UI decides what to show.
    pub fn take_failures(&mut self) -> Vec<TrainerFailure> {
        self.drain();
        std::mem::take(&mut self.failures)
    }

    fn set_state(&mut self, state: TrainerConnectionState) {
        if self.state != state {
            self.state = state;
            self.pending.push(TrainerEvent::ConnectionState(state));
        }
    }

    fn drain(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                WorkerEvent::State(s) => {
                    // After dispose, a late report from the worker must not resurrect the trainer.
                    if self.commands.is_some() {
                        self.set_state(s);
                    }
                }
                WorkerEvent::Telemetry(t) => self.pending.push(TrainerEvent::Telemetry(t)),
                WorkerEvent::Failure(f) => {
                    log::warn!("trainer: {f}");
                    self.failures.push(f);
                }
            }
        }
    }

    fn send(&self, command: Command) -> Result<(), TrainerError> {
        let commands = self
            .commands
            .as_ref()
            .ok_or_else(|| TrainerError::new("connect() must be called before this"))?;
        commands
            .send(command)
            .map_err(|_| TrainerError::new("the trainer connection has ended"))
    }
}

impl TrainerControl for BleTrainer {
    fn connection_state(&self) -> TrainerConnectionState {
        self.state
    }

    fn connect(&mut self, _now: Duration) -> Result<(), TrainerError> {
        let Some(spawn) = self.spawn.take() else {
            return if self.commands.is_some() {
                Ok(())
            } else {
                Err(TrainerError::new(
                    "this trainer was disposed; create a new one to reconnect",
                ))
            };
        };
        let (commands_tx, commands_rx) = tokio_mpsc::unbounded_channel();
        self.commands = Some(commands_tx);
        self.set_state(TrainerConnectionState::Connecting);
        spawn(commands_rx, self.events_tx.clone());
        Ok(())
    }

    fn set_simulation_parameters(
        &mut self,
        params: SimulationParameters,
        _now: Duration,
    ) -> Result<(), TrainerError> {
        self.send(Command::SetSimulationParameters(params))
    }

    fn start(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.send(Command::StartOrResume)
    }

    fn stop(&mut self, _now: Duration) -> Result<(), TrainerError> {
        self.send(Command::Pause)
    }

    fn poll(&mut self, _now: Duration) -> Vec<TrainerEvent> {
        self.drain();
        std::mem::take(&mut self.pending)
    }

    fn dispose(&mut self) {
        self.spawn = None;
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(Command::Disconnect);
        }
        self.drain();
        self.set_state(TrainerConnectionState::Disconnected);
    }
}

impl Drop for BleTrainer {
    fn drop(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(Command::Disconnect);
        }
    }
}

async fn run_worker<C: TrainerConnection + Clone>(
    connect: Pin<Box<dyn Future<Output = Result<C, TrainerFailure>> + Send>>,
    mut commands: tokio_mpsc::UnboundedReceiver<Command>,
    events: mpsc::Sender<WorkerEvent>,
) {
    let send = |e| {
        let _ = events.send(e);
    };
    let connection = match connect.await {
        Ok(c) => c,
        Err(failure) => {
            send(WorkerEvent::Failure(failure));
            send(WorkerEvent::State(TrainerConnectionState::Disconnected));
            return;
        }
    };
    send(WorkerEvent::State(TrainerConnectionState::Connected));

    // Telemetry on its own clone, so a write awaiting its ack never delays a report.
    let closing = Arc::new(AtomicBool::new(false));
    let telemetry = tokio::spawn({
        let mut connection = connection.clone();
        let events = events.clone();
        let closing = closing.clone();
        async move {
            let lost = |failure: Option<TrainerFailure>| {
                if let Some(f) = failure {
                    let _ = events.send(WorkerEvent::Failure(f));
                }
                if !closing.load(Ordering::SeqCst) {
                    let _ = events.send(WorkerEvent::State(TrainerConnectionState::ConnectionLost));
                }
            };
            loop {
                match connection.next_telemetry().await {
                    Ok(Some(t)) => {
                        if events
                            .send(WorkerEvent::Telemetry(convert::telemetry(&t)))
                            .is_err()
                        {
                            return;
                        }
                    }
                    // The stream ending is how a dropped link shows up.
                    Ok(None) => return lost(None),
                    Err(e) => return lost(Some(e.into())),
                }
            }
        }
    });

    while let Some(command) = commands.recv().await {
        let result = match command {
            Command::SetSimulationParameters(p) => {
                connection.set_sim_params(&convert::sim_params(&p)).await
            }
            Command::StartOrResume => connection.start_or_resume().await,
            Command::Pause => connection.stop_or_pause(true).await,
            Command::Disconnect => break,
        };
        if let Err(e) = result {
            let mut failure = TrainerFailure::from(e);
            if failure.kind == TrainerFailureKind::Other {
                failure.kind = TrainerFailureKind::WriteFailed;
            }
            send(WorkerEvent::Failure(failure));
        }
    }
    // Disposed, or the BleTrainer was dropped.
    closing.store(true, Ordering::SeqCst);
    telemetry.abort();
    if let Err(e) = connection.disconnect().await {
        log::warn!("trainer: disconnect failed: {e}");
    }
    send(WorkerEvent::State(TrainerConnectionState::Disconnected));
}
