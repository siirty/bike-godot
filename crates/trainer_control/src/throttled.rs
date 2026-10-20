use std::time::Duration;

use crate::{
    ManualPowerSource, SimulationParameters, TrainerConnectionState, TrainerControl, TrainerError,
    TrainerEvent,
};

/// The default minimum interval between SIM writes ("~1 Hz max").
pub const DEFAULT_MIN_WRITE_INTERVAL: Duration = Duration::from_secs(1);

/// The default grade change considered material: 0.2 %.
pub const DEFAULT_MIN_GRADE_DELTA: f64 = 0.002;

/// The one owner of the SIM-write policy: write on a materially changed grade, at most ~1 Hz.
/// Compose it around any trainer; never re-implement the rule in a caller.
///
/// A write is dropped only when it would be a near-no-op: same non-grade parameters and a grade
/// within `min_grade_delta` of the last write. The first write, and any change to wind, Crr or CdA,
/// always go through at once. A material grade change that arrives too soon is kept (newest wins)
/// and flushed by [`TrainerControl::poll`] once the window opens, so the trainer converges on the
/// true grade.
///
/// The Rust core's `MIN_SIM_PARAMS_WRITE_INTERVAL` is a backstop that *delays*; this *drops*
/// superseded writes, which is what the policy asks for.
#[derive(Debug)]
pub struct ThrottledTrainerControl<T> {
    inner: T,
    min_write_interval: Duration,
    min_grade_delta: f64,
    last_written: Option<(SimulationParameters, Duration)>,
    pending: Option<SimulationParameters>,
}

impl<T: TrainerControl> ThrottledTrainerControl<T> {
    /// Wraps `inner` with the default policy.
    pub fn new(inner: T) -> Self {
        Self::with_policy(inner, DEFAULT_MIN_WRITE_INTERVAL, DEFAULT_MIN_GRADE_DELTA)
    }

    /// Wraps `inner` with an explicit policy.
    pub fn with_policy(inner: T, min_write_interval: Duration, min_grade_delta: f64) -> Self {
        Self {
            inner,
            min_write_interval,
            min_grade_delta,
            last_written: None,
            pending: None,
        }
    }

    /// The parameters most recently written through, if any.
    pub fn last_written_parameters(&self) -> Option<SimulationParameters> {
        self.last_written.map(|(p, _)| p)
    }

    /// The wrapped trainer.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// The wrapped trainer, mutably. Writing SIM parameters through this bypasses the policy.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Unwraps the trainer. A write still pending in the throttle is dropped.
    pub fn into_inner(self) -> T {
        self.inner
    }

    fn write(&mut self, params: SimulationParameters, now: Duration) -> Result<(), TrainerError> {
        self.last_written = Some((params, now));
        self.pending = None;
        self.inner.set_simulation_parameters(params, now)
    }

    fn flush_due(&mut self, now: Duration) -> Result<(), TrainerError> {
        if let (Some(pending), Some((_, at))) = (self.pending, self.last_written) {
            if now.saturating_sub(at) >= self.min_write_interval {
                return self.write(pending, now);
            }
        }
        Ok(())
    }
}

impl<T: TrainerControl> TrainerControl for ThrottledTrainerControl<T> {
    fn connection_state(&self) -> TrainerConnectionState {
        self.inner.connection_state()
    }

    fn connect(&mut self, now: Duration) -> Result<(), TrainerError> {
        self.inner.connect(now)
    }

    fn set_simulation_parameters(
        &mut self,
        params: SimulationParameters,
        now: Duration,
    ) -> Result<(), TrainerError> {
        let Some((last, written_at)) = self.last_written else {
            return self.write(params, now);
        };
        if params.with_grade(last.grade_fraction) != last {
            return self.write(params, now);
        }
        if (params.grade_fraction - last.grade_fraction).abs() < self.min_grade_delta {
            return Ok(());
        }
        if now.saturating_sub(written_at) >= self.min_write_interval {
            return self.write(params, now);
        }
        self.pending = Some(params);
        Ok(())
    }

    fn start(&mut self, now: Duration) -> Result<(), TrainerError> {
        self.inner.start(now)
    }

    fn stop(&mut self, now: Duration) -> Result<(), TrainerError> {
        self.inner.stop(now)
    }

    /// Flushes a due pending write, then polls the wrapped trainer.
    fn poll(&mut self, now: Duration) -> Vec<TrainerEvent> {
        // A failed flush keeps nothing pending; the next material change writes again.
        let _ = self.flush_due(now);
        self.inner.poll(now)
    }

    fn dispose(&mut self) {
        self.pending = None;
        self.inner.dispose();
    }

    fn manual_power_source(&mut self) -> Option<&mut dyn ManualPowerSource> {
        self.inner.manual_power_source()
    }
}
