//! The Bluetooth worker against a fake link implementing `trainer_core`'s connection trait: no
//! adapter, no hardware. Real time is involved (a tokio worker), so waits are bounded polls.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use ble_trainer::{BleTrainer, TrainerFailure, TrainerFailureKind};
use tokio::sync::mpsc;
use trainer_control::{SimulationParameters, TrainerConnectionState, TrainerControl, TrainerEvent};
use trainer_core::ble::BleError;
use trainer_core::{
    SimParams, TrainerCapabilities, TrainerConnection, TrainerError, TrainerTelemetry,
};

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Sim(f64),
    Start,
    Pause(bool),
    Disconnect,
    Forbidden(&'static str),
}

#[derive(Clone)]
struct FakeLink {
    calls: Arc<Mutex<Vec<Call>>>,
    telemetry: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Option<TrainerTelemetry>>>>,
    fail_writes: bool,
}

#[async_trait]
impl TrainerConnection for FakeLink {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn capabilities(&self) -> TrainerCapabilities {
        TrainerCapabilities::default()
    }
    async fn is_connected(&self) -> Result<bool, TrainerError> {
        Ok(true)
    }
    async fn disconnect(&self) -> Result<(), TrainerError> {
        self.calls.lock().unwrap().push(Call::Disconnect);
        Ok(())
    }
    async fn next_telemetry(&mut self) -> Result<Option<TrainerTelemetry>, TrainerError> {
        Ok(self.telemetry.lock().await.recv().await.flatten())
    }
    async fn set_sim_params(&self, params: &SimParams) -> Result<(), TrainerError> {
        self.calls
            .lock()
            .unwrap()
            .push(Call::Sim(params.grade.as_fraction()));
        if self.fail_writes {
            return Err(BleError::NoAdapterAvailable.into());
        }
        Ok(())
    }
    async fn set_target_power(&self, _: i32) -> Result<(), TrainerError> {
        self.calls
            .lock()
            .unwrap()
            .push(Call::Forbidden("set_target_power"));
        Ok(())
    }
    async fn set_target_resistance_level(&self, _: f64) -> Result<(), TrainerError> {
        self.calls
            .lock()
            .unwrap()
            .push(Call::Forbidden("resistance"));
        Ok(())
    }
    async fn set_wheel_circumference(&self, _: f64) -> Result<(), TrainerError> {
        self.calls.lock().unwrap().push(Call::Forbidden("wheel"));
        Ok(())
    }
    async fn reset(&self) -> Result<(), TrainerError> {
        self.calls.lock().unwrap().push(Call::Forbidden("reset"));
        Ok(())
    }
    async fn start_or_resume(&self) -> Result<(), TrainerError> {
        self.calls.lock().unwrap().push(Call::Start);
        Ok(())
    }
    async fn stop_or_pause(&self, pause: bool) -> Result<(), TrainerError> {
        self.calls.lock().unwrap().push(Call::Pause(pause));
        Ok(())
    }
}

struct Harness {
    trainer: BleTrainer,
    calls: Arc<Mutex<Vec<Call>>>,
    telemetry: mpsc::UnboundedSender<Option<TrainerTelemetry>>,
}

fn harness(fail_writes: bool) -> Harness {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (telemetry, rx) = mpsc::unbounded_channel();
    let link = FakeLink {
        calls: calls.clone(),
        telemetry: Arc::new(tokio::sync::Mutex::new(rx)),
        fail_writes,
    };
    Harness {
        trainer: BleTrainer::with_link(async move { Ok::<_, TrainerFailure>(link) }),
        calls,
        telemetry,
    }
}

/// Polls until `done` holds or two seconds pass, collecting events.
fn poll_until(
    trainer: &mut BleTrainer,
    mut done: impl FnMut(&[TrainerEvent]) -> bool,
) -> Vec<TrainerEvent> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut all = Vec::new();
    while Instant::now() < deadline {
        all.extend(trainer.poll(Duration::ZERO));
        if done(&all) {
            return all;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("timed out; saw {all:?}");
}

fn has_state(events: &[TrainerEvent], state: TrainerConnectionState) -> bool {
    events.contains(&TrainerEvent::ConnectionState(state))
}

#[test]
fn connects_applies_queued_writes_in_order_and_forwards_telemetry() {
    let mut h = harness(false);
    h.trainer.connect(Duration::ZERO).unwrap();
    assert_eq!(
        h.trainer.connection_state(),
        TrainerConnectionState::Connecting
    );
    // Issued before the link is up: queued, not lost.
    h.trainer
        .set_simulation_parameters(
            SimulationParameters {
                grade_fraction: 0.03,
                ..SimulationParameters::default()
            },
            Duration::ZERO,
        )
        .unwrap();
    h.trainer.start(Duration::ZERO).unwrap();

    let events = poll_until(&mut h.trainer, |e| {
        has_state(e, TrainerConnectionState::Connected)
    });
    assert!(has_state(&events, TrainerConnectionState::Connecting));

    h.telemetry
        .send(Some(TrainerTelemetry {
            speed_mps: 9.5,
            power_w: 210.0,
            total_distance_m: 12.0,
            elapsed_time_s: None,
            ..TrainerTelemetry::default()
        }))
        .unwrap();
    let events = poll_until(&mut h.trainer, |e| {
        e.iter().any(|e| matches!(e, TrainerEvent::Telemetry(_)))
    });
    let TrainerEvent::Telemetry(t) = events
        .iter()
        .find(|e| matches!(e, TrainerEvent::Telemetry(_)))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        (t.speed_ms, t.power_watts, t.total_distance_m),
        (9.5, 210.0, 12.0)
    );
    assert_eq!(
        t.elapsed_session_time, None,
        "an absent FTMS elapsed time stays absent"
    );

    h.trainer.stop(Duration::ZERO).unwrap();
    h.trainer.dispose();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !h.calls.lock().unwrap().contains(&Call::Disconnect) {
        assert!(Instant::now() < deadline, "disconnected");
        std::thread::sleep(Duration::from_millis(2));
    }
    let calls = h.calls.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec![
            Call::Sim(0.03),
            Call::Start,
            Call::Pause(true),
            Call::Disconnect
        ]
    );
    assert_eq!(
        h.trainer.connection_state(),
        TrainerConnectionState::Disconnected
    );
}

#[test]
fn the_link_ending_is_reported_as_connection_lost() {
    let mut h = harness(false);
    h.trainer.connect(Duration::ZERO).unwrap();
    poll_until(&mut h.trainer, |e| {
        has_state(e, TrainerConnectionState::Connected)
    });
    h.telemetry.send(None).unwrap();
    poll_until(&mut h.trainer, |e| {
        has_state(e, TrainerConnectionState::ConnectionLost)
    });
}

#[test]
fn a_refused_write_is_a_failure_not_a_dead_trainer() {
    let mut h = harness(true);
    h.trainer.connect(Duration::ZERO).unwrap();
    h.trainer
        .set_simulation_parameters(SimulationParameters::default(), Duration::ZERO)
        .unwrap();
    poll_until(&mut h.trainer, |e| {
        has_state(e, TrainerConnectionState::Connected)
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    let failures = loop {
        let f = h.trainer.take_failures();
        if !f.is_empty() {
            break f;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(failures[0].kind, TrainerFailureKind::NoAdapter);
    assert_eq!(
        h.trainer.connection_state(),
        TrainerConnectionState::Connected
    );
}

#[test]
fn a_failed_connect_returns_to_disconnected_with_the_reason() {
    let mut trainer =
        BleTrainer::with_link(async { Err::<FakeLink, _>(BleError::NoAdapterAvailable.into()) });
    trainer.connect(Duration::ZERO).unwrap();
    poll_until(&mut trainer, |e| {
        has_state(e, TrainerConnectionState::Disconnected)
    });
    assert_eq!(
        trainer.take_failures()[0].kind,
        TrainerFailureKind::NoAdapter
    );
}

#[test]
fn writes_before_connect_and_reconnecting_after_dispose_are_errors() {
    let mut h = harness(false);
    assert!(h.trainer.start(Duration::ZERO).is_err());
    h.trainer.connect(Duration::ZERO).unwrap();
    h.trainer.dispose();
    assert!(h.trainer.connect(Duration::ZERO).is_err());
    assert!(h
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|c| !matches!(c, Call::Forbidden(_))));
}
