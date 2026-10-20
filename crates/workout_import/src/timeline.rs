//! The canonical reading of a [`WorkoutPlan`] as a function of elapsed time.

use std::time::Duration;

use crate::error::{OutOfRange, WorkoutImportError};
use crate::model::{IntervalRepeat, PowerTarget, WorkoutPlan, WorkoutSegment};

/// Built once from a plan, then queried per tick by binary search over precomputed segment ends.
///
/// **This is the one traversal implementation.** Anything that needs "which segment is the rider
/// in, and for how much longer" goes through here.
///
/// ## Boundary convention
///
/// A segment owns the half-open interval `[start, end)`, so at an exact interior boundary the
/// *later* segment wins. The final segment owns the closed interval `[start, end]`, so
/// `t == total_duration` resolves to the last segment with zero time remaining.
#[derive(Debug, Clone)]
pub struct WorkoutTimeline {
    plan: WorkoutPlan,
    segment_ends_micros: Vec<i64>,
}

impl WorkoutTimeline {
    /// Builds a timeline for `plan`. Fails if the plan has no segments, so an empty plan is
    /// rejected at construction rather than inside a render loop.
    pub fn new(plan: WorkoutPlan) -> Result<Self, WorkoutImportError> {
        if plan.segments.is_empty() {
            return Err(WorkoutImportError::new(
                "a workout plan must have at least one segment",
            ));
        }
        let mut elapsed = 0_i64;
        let segment_ends_micros = plan
            .segments
            .iter()
            .map(|segment| {
                elapsed += micros(segment.duration());
                elapsed
            })
            .collect();
        Ok(Self {
            plan,
            segment_ends_micros,
        })
    }

    /// The plan this timeline reads.
    pub fn plan(&self) -> &WorkoutPlan {
        &self.plan
    }

    /// The sum of every segment's duration.
    pub fn total_duration(&self) -> Duration {
        from_micros(self.total_micros())
    }

    fn total_micros(&self) -> i64 {
        *self
            .segment_ends_micros
            .last()
            .expect("non-empty by construction")
    }

    /// The index of the segment containing `t`.
    pub fn index_at(&self, t: Duration) -> Result<usize, OutOfRange> {
        let t = self.check_in_range(t)?;
        let (mut lo, mut hi) = (0, self.segment_ends_micros.len() - 1);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if t < self.segment_ends_micros[mid] {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        Ok(lo)
    }

    /// The segment containing `t`.
    pub fn segment_at(&self, t: Duration) -> Result<&WorkoutSegment, OutOfRange> {
        Ok(&self.plan.segments[self.index_at(t)?])
    }

    /// How long the segment containing `t` still has to run. Zero at `t == total_duration`.
    pub fn time_remaining_at(&self, t: Duration) -> Result<Duration, OutOfRange> {
        let index = self.index_at(t)?;
        Ok(from_micros(self.segment_ends_micros[index] - micros(t)))
    }

    /// How far into the segment containing `t` the rider is.
    pub fn time_elapsed_in_segment_at(&self, t: Duration) -> Result<Duration, OutOfRange> {
        let index = self.index_at(t)?;
        Ok(from_micros(micros(t) - self.segment_start_micros(index)))
    }

    /// The target power at `t`, resolved against `ftp_watts`. `None` inside a free ride.
    pub fn power_at(&self, t: Duration, ftp_watts: f64) -> Result<Option<f64>, OutOfRange> {
        let index = self.index_at(t)?;
        let t_in_segment = micros(t) - self.segment_start_micros(index);
        Ok(power_within_segment(
            &self.plan.segments[index],
            t_in_segment,
            ftp_watts,
        ))
    }

    fn segment_start_micros(&self, index: usize) -> i64 {
        if index == 0 {
            0
        } else {
            self.segment_ends_micros[index - 1]
        }
    }

    fn check_in_range(&self, t: Duration) -> Result<i64, OutOfRange> {
        let t_micros = micros(t);
        let total = self.total_micros();
        if t_micros > total {
            return Err(OutOfRange {
                t_micros,
                total_micros: total,
            });
        }
        Ok(t_micros)
    }
}

fn power_within_segment(segment: &WorkoutSegment, t_in_segment: i64, ftp: f64) -> Option<f64> {
    match segment {
        WorkoutSegment::SteadyState { target_power, .. } => Some(target_power.resolve_watts(ftp)),
        WorkoutSegment::Ramp {
            duration,
            start_power,
            end_power,
            ..
        } => Some(lerp(
            start_power.resolve_watts(ftp),
            end_power.resolve_watts(ftp),
            fraction(t_in_segment, micros(*duration)),
        )),
        WorkoutSegment::IntervalRepeat(block) => {
            Some(power_within_interval_repeat(block, t_in_segment, ftp))
        }
        WorkoutSegment::FreeRide { .. } => None,
    }
}

fn power_within_interval_repeat(block: &IntervalRepeat, t_in_segment: i64, ftp: f64) -> f64 {
    let has_off_phase = !block.off_duration().is_zero();
    // The off powers are guaranteed by IntervalRepeat::new whenever the off phase is non-empty.
    let off = |p: Option<PowerTarget>| p.expect("IntervalRepeat invariant: off powers present");

    if t_in_segment >= micros(block.duration()) {
        let end_power = if has_off_phase {
            off(block.off_end_power())
        } else {
            block.on_end_power()
        };
        return end_power.resolve_watts(ftp);
    }

    let on = micros(block.on_duration());
    let cycle = on + micros(block.off_duration());
    let cycle_elapsed = t_in_segment % cycle;

    if cycle_elapsed < on {
        return lerp(
            block.on_start_power().resolve_watts(ftp),
            block.on_end_power().resolve_watts(ftp),
            fraction(cycle_elapsed, on),
        );
    }
    lerp(
        off(block.off_start_power()).resolve_watts(ftp),
        off(block.off_end_power()).resolve_watts(ftp),
        fraction(cycle_elapsed - on, micros(block.off_duration())),
    )
}

fn fraction(elapsed_micros: i64, segment_micros: i64) -> f64 {
    if segment_micros == 0 {
        0.0
    } else {
        elapsed_micros as f64 / segment_micros as f64
    }
}

fn lerp(start: f64, end: f64, fraction: f64) -> f64 {
    start + (end - start) * fraction
}

/// A duration as whole microseconds, the timeline resolution.
pub(crate) fn micros(d: Duration) -> i64 {
    i64::try_from(d.as_micros()).expect("workout durations fit in i64 microseconds")
}

fn from_micros(m: i64) -> Duration {
    Duration::from_micros(u64::try_from(m).expect("non-negative by construction"))
}
