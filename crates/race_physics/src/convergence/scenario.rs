use std::time::Duration;

use crate::RiderPhysicalParams;

/// One piece of a scripted, piecewise-constant input trace. The values are held for exactly
/// `duration` and change at the boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputSegment {
    /// How long these inputs are held.
    pub duration: Duration,
    /// Rider power, in watts.
    pub power_watts: f64,
    /// Braking force, in newtons.
    pub braking_force_n: f64,
    /// Grade as rise/run.
    pub grade_fraction: f64,
    /// Headwind, in m/s. Negative is a tailwind.
    pub headwind_ms: f64,
    /// Aero drag multiplier. `1` is clean air.
    pub aerodynamic_drag_multiplier: f64,
}

impl InputSegment {
    /// Coasting on flat ground in still, clean air for `seconds`.
    pub const fn coast(seconds: u64) -> Self {
        Self {
            duration: Duration::from_secs(seconds),
            power_watts: 0.0,
            braking_force_n: 0.0,
            grade_fraction: 0.0,
            headwind_ms: 0.0,
            aerodynamic_drag_multiplier: 1.0,
        }
    }

    /// The same segment with rider power.
    pub const fn power(mut self, watts: f64) -> Self {
        self.power_watts = watts;
        self
    }

    /// The same segment with braking.
    pub const fn braking(mut self, newtons: f64) -> Self {
        self.braking_force_n = newtons;
        self
    }

    /// The same segment on a grade.
    pub const fn grade(mut self, fraction: f64) -> Self {
        self.grade_fraction = fraction;
        self
    }

    /// The same segment in wind.
    pub const fn headwind(mut self, ms: f64) -> Self {
        self.headwind_ms = ms;
        self
    }

    /// The same segment with shelter.
    pub const fn drag_multiplier(mut self, multiplier: f64) -> Self {
        self.aerodynamic_drag_multiplier = multiplier;
        self
    }
}

/// One rider's scripted run.
#[derive(Debug, Clone, PartialEq)]
pub struct RiderTrack {
    /// Short identifier.
    pub name: &'static str,
    /// Fixed physical parameters.
    pub rider: RiderPhysicalParams,
    /// The trace, in order.
    pub segments: Vec<InputSegment>,
    /// Speed at `t = 0`, in m/s.
    pub initial_speed_ms: f64,
}

impl RiderTrack {
    /// Total scripted duration.
    pub fn duration(&self) -> Duration {
        self.segments.iter().map(|s| s.duration).sum()
    }
}

/// One convergence case: one or two independently integrated riders. Two riders measure drift in
/// the gap between them, which is what a race spends.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvergenceScenario {
    /// Stable identifier; the README table is keyed on it.
    pub name: &'static str,
    /// What numerical behaviour this case exposes.
    pub what: &'static str,
    /// The riders.
    pub tracks: Vec<RiderTrack>,
}

impl ConvergenceScenario {
    /// The scenario's duration (equal for every track; see [`Self::assert_well_formed`]).
    pub fn duration(&self) -> Duration {
        self.tracks[0].duration()
    }

    /// Fails unless every track has the same duration and every segment boundary and the total
    /// duration fall on a step boundary of every step in `steps`. Otherwise the comparison would
    /// measure input aliasing rather than integration error.
    pub fn assert_well_formed(&self, steps: &[Duration]) -> Result<(), String> {
        let duration = self.duration();
        for track in &self.tracks {
            if track.duration() != duration {
                return Err(format!(
                    "scenario \"{}\": track \"{}\" runs for {:?}, but \"{}\" runs for {duration:?}",
                    self.name,
                    track.name,
                    track.duration(),
                    self.tracks[0].name
                ));
            }
            let mut elapsed = Duration::ZERO;
            for segment in &track.segments {
                elapsed += segment.duration;
                for step in steps {
                    if !elapsed.as_micros().is_multiple_of(step.as_micros()) {
                        return Err(format!(
                            "scenario \"{}\": track \"{}\" changes inputs at {elapsed:?}, which is \
                             not a whole number of {step:?} steps",
                            self.name, track.name
                        ));
                    }
                }
            }
        }
        for step in steps {
            if !duration.as_micros().is_multiple_of(step.as_micros()) {
                return Err(format!(
                    "scenario \"{}\" runs for {duration:?}, which is not a whole number of \
                     {step:?} steps",
                    self.name
                ));
            }
        }
        Ok(())
    }
}
