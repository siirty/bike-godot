//! The workout model: power targets, segments, and plans.

use std::time::Duration;

use crate::error::WorkoutImportError;
use crate::timeline::WorkoutTimeline;

/// A segment's target power in the unit its source file expressed it in, resolved to watts only on
/// demand against the rider's FTP *at the time of the query*.
///
/// Keeping the source unit means a rider who changes FTP after importing a workout keeps the same
/// relative effort: a `.zwo` segment at 75% FTP still means 75% of whatever FTP is current.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PowerTarget {
    /// A fraction of FTP (`1.0` == 100% FTP) — `.zwo`'s and `.mrc`'s native unit.
    PercentFtp(f64),
    /// Literal watts — `.erg`'s native unit. Ignores FTP entirely.
    AbsoluteWatts(f64),
}

impl PowerTarget {
    /// Resolves this target to watts given the rider's current FTP.
    pub fn resolve_watts(self, ftp_watts: f64) -> f64 {
        match self {
            PowerTarget::PercentFtp(fraction) => fraction * ftp_watts,
            PowerTarget::AbsoluteWatts(watts) => watts,
        }
    }
}

/// A repeated on/off interval block — `.zwo`'s `IntervalsT` — kept as one unit so it can be
/// presented as one ("4x5min @ 100/50% FTP").
///
/// Fields are private so the invariant (an off phase with a duration needs both off powers, the
/// block repeats at least once) is enforced by [`IntervalRepeat::new`] on every construction path.
#[derive(Debug, Clone, PartialEq)]
pub struct IntervalRepeat {
    repeat: u32,
    on_duration: Duration,
    on_start_power: PowerTarget,
    on_end_power: PowerTarget,
    off_duration: Duration,
    off_start_power: Option<PowerTarget>,
    off_end_power: Option<PowerTarget>,
    on_cadence: Option<i64>,
    off_cadence: Option<i64>,
}

/// The fields of an [`IntervalRepeat`], as a caller supplies them before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct IntervalRepeatSpec {
    /// How many on/off cycles. At least 1.
    pub repeat: u32,
    /// Length of each "on" phase.
    pub on_duration: Duration,
    /// Power at the start of each "on" phase.
    pub on_start_power: PowerTarget,
    /// Power at the end of each "on" phase.
    pub on_end_power: PowerTarget,
    /// Length of each "off" phase; zero for back-to-back "on" bursts.
    pub off_duration: Duration,
    /// Power at the start of each "off" phase. Required when `off_duration > 0`.
    pub off_start_power: Option<PowerTarget>,
    /// Power at the end of each "off" phase. Required when `off_duration > 0`.
    pub off_end_power: Option<PowerTarget>,
    /// Target cadence during "on" phases, in RPM.
    pub on_cadence: Option<i64>,
    /// Target cadence during "off" phases, in RPM.
    pub off_cadence: Option<i64>,
}

impl IntervalRepeat {
    /// Validates `spec` and builds the block.
    ///
    /// Fails if an off phase with a positive duration lacks either off power, or if `repeat < 1`.
    pub fn new(spec: IntervalRepeatSpec) -> Result<Self, WorkoutImportError> {
        if !spec.off_duration.is_zero()
            && (spec.off_start_power.is_none() || spec.off_end_power.is_none())
        {
            return Err(WorkoutImportError::new(
                "an interval-repeat off phase (offDuration > 0) requires both offStartPower and \
                 offEndPower",
            ));
        }
        if spec.repeat < 1 {
            return Err(WorkoutImportError::new(
                "an interval-repeat segment must repeat at least once",
            ));
        }
        Ok(Self {
            repeat: spec.repeat,
            on_duration: spec.on_duration,
            on_start_power: spec.on_start_power,
            on_end_power: spec.on_end_power,
            off_duration: spec.off_duration,
            off_start_power: spec.off_start_power,
            off_end_power: spec.off_end_power,
            on_cadence: spec.on_cadence,
            off_cadence: spec.off_cadence,
        })
    }

    /// How many on/off cycles.
    pub fn repeat(&self) -> u32 {
        self.repeat
    }
    /// Length of each "on" phase.
    pub fn on_duration(&self) -> Duration {
        self.on_duration
    }
    /// Power at the start of each "on" phase.
    pub fn on_start_power(&self) -> PowerTarget {
        self.on_start_power
    }
    /// Power at the end of each "on" phase.
    pub fn on_end_power(&self) -> PowerTarget {
        self.on_end_power
    }
    /// Length of each "off" phase.
    pub fn off_duration(&self) -> Duration {
        self.off_duration
    }
    /// Power at the start of each "off" phase, if there is one.
    pub fn off_start_power(&self) -> Option<PowerTarget> {
        self.off_start_power
    }
    /// Power at the end of each "off" phase, if there is one.
    pub fn off_end_power(&self) -> Option<PowerTarget> {
        self.off_end_power
    }
    /// Target cadence during "on" phases.
    pub fn on_cadence(&self) -> Option<i64> {
        self.on_cadence
    }
    /// Target cadence during "off" phases.
    pub fn off_cadence(&self) -> Option<i64> {
        self.off_cadence
    }
    /// Total length: `(on + off) * repeat`.
    pub fn duration(&self) -> Duration {
        (self.on_duration + self.off_duration) * self.repeat
    }
}

/// One scheduled block of a [`WorkoutPlan`].
#[derive(Debug, Clone, PartialEq)]
pub enum WorkoutSegment {
    /// A constant target held for the whole segment.
    SteadyState {
        /// How long the segment lasts.
        duration: Duration,
        /// The target power.
        target_power: PowerTarget,
        /// Target cadence in RPM, if the file gave one.
        cadence: Option<i64>,
    },
    /// A linear ramp from `start_power` to `end_power`. Also every breakpoint span of `.erg`/`.mrc`.
    Ramp {
        /// How long the segment lasts.
        duration: Duration,
        /// Power at the start of the ramp.
        start_power: PowerTarget,
        /// Power at the end of the ramp.
        end_power: PowerTarget,
        /// Target cadence in RPM, if the file gave one.
        cadence: Option<i64>,
    },
    /// A repeated on/off block.
    IntervalRepeat(IntervalRepeat),
    /// A rider-controlled block with no target power — `.zwo`'s `FreeRide`.
    FreeRide {
        /// How long the segment lasts.
        duration: Duration,
        /// Target cadence in RPM, if the file gave one.
        cadence: Option<i64>,
    },
}

impl WorkoutSegment {
    /// How long this segment lasts.
    pub fn duration(&self) -> Duration {
        match self {
            WorkoutSegment::SteadyState { duration, .. }
            | WorkoutSegment::Ramp { duration, .. }
            | WorkoutSegment::FreeRide { duration, .. } => *duration,
            WorkoutSegment::IntervalRepeat(block) => block.duration(),
        }
    }

    /// The persisted discriminator for this segment kind.
    pub fn kind_tag(&self) -> &'static str {
        match self {
            WorkoutSegment::SteadyState { .. } => "steadyState",
            WorkoutSegment::Ramp { .. } => "ramp",
            WorkoutSegment::IntervalRepeat(_) => "intervalRepeat",
            WorkoutSegment::FreeRide { .. } => "freeRide",
        }
    }
}

/// A structured workout: an ordered sequence of segments, plus optional metadata.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorkoutPlan {
    /// The title. `.zwo` always has one; `.erg`/`.mrc` do not, so this is `None` for them.
    pub name: Option<String>,
    /// The author, if the file names one.
    pub author: Option<String>,
    /// A description (`.zwo` `<description>`, `.erg`/`.mrc` `DESCRIPTION`).
    pub description: Option<String>,
    /// The segments, in order.
    pub segments: Vec<WorkoutSegment>,
}

impl WorkoutPlan {
    /// The sum of every segment's duration.
    pub fn total_duration(&self) -> Duration {
        self.segments.iter().map(WorkoutSegment::duration).sum()
    }

    /// The target power at `t`, resolved against `ftp_watts`. A convenience that builds a
    /// [`WorkoutTimeline`] per call; hold a timeline instead when querying repeatedly.
    ///
    /// `Ok(None)` inside a free ride, which has no target by design.
    pub fn power_at(&self, t: Duration, ftp_watts: f64) -> Result<Option<f64>, PowerAtError> {
        let timeline = WorkoutTimeline::new(self.clone()).map_err(PowerAtError::Invalid)?;
        timeline
            .power_at(t, ftp_watts)
            .map_err(PowerAtError::OutOfRange)
    }
}

/// Why [`WorkoutPlan::power_at`] failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PowerAtError {
    /// The plan has no segments.
    #[error("{0}")]
    Invalid(WorkoutImportError),
    /// The time is outside the plan.
    #[error("{0}")]
    OutOfRange(crate::error::OutOfRange),
}
