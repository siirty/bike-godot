use std::time::Duration;

use super::scenario::{ConvergenceScenario, RiderTrack};
use crate::{
    step_longitudinal_dynamics, EnvironmentParams, LongitudinalDynamicsConfig,
    LongitudinalStepInput, PhysicsError,
};

/// The times at which a candidate and the reference are compared. Every candidate step is a whole
/// number of grid ticks and the grid a whole number of reference steps, so no interpolation is
/// ever needed.
pub const COMPARISON_GRID: Duration = Duration::from_millis(10);

/// One track's trajectory, sampled every `sample_interval_micros`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackTrajectory {
    /// Sample times, in microseconds, ascending.
    pub times_micros: Vec<i64>,
    /// Speed at each sample, in m/s.
    pub speed_ms: Vec<f64>,
    /// Cumulative distance at each sample, in metres.
    pub distance_m: Vec<f64>,
    /// The step this was integrated with.
    pub step: Duration,
    /// The coarser of the step and the comparison grid, in microseconds.
    pub sample_interval_micros: i64,
    /// When the rider first stopped after having moved, at full step resolution.
    pub stopped_at: Option<Duration>,
    /// Cumulative distance at [`Self::stopped_at`].
    pub stopped_at_distance_m: Option<f64>,
}

impl TrackTrajectory {
    /// The index of the sample at exactly `micros`.
    ///
    /// # Panics
    /// If there is no sample at that time: the corpus and grid have drifted apart.
    pub fn index_at_micros(&self, micros: i64) -> usize {
        assert!(
            micros % self.sample_interval_micros == 0,
            "{micros} is not a sample time of a trajectory recorded every {} µs",
            self.sample_interval_micros
        );
        (micros / self.sample_interval_micros) as usize
    }

    /// Final speed, in m/s.
    pub fn final_speed_ms(&self) -> f64 {
        *self
            .speed_ms
            .last()
            .expect("a trajectory always has its t=0 sample")
    }

    /// Final distance, in metres.
    pub fn final_distance_m(&self) -> f64 {
        *self
            .distance_m
            .last()
            .expect("a trajectory always has its t=0 sample")
    }
}

/// Integrates `track` with a fixed `step`, sampling on `grid`. The only place the corpus calls the
/// integrator, exactly once per step: the reference differs from a candidate only in `step`.
pub fn run_track(
    track: &RiderTrack,
    step: Duration,
    config: &LongitudinalDynamicsConfig,
    grid: Duration,
) -> Result<TrackTrajectory, PhysicsError> {
    let step_micros = step.as_micros() as i64;
    let grid_micros = grid.as_micros() as i64;
    if step_micros <= 0 {
        return Err(bad_step(step, "must be positive"));
    }
    if grid_micros % step_micros != 0 && step_micros % grid_micros != 0 {
        return Err(bad_step(
            step,
            "must be a whole multiple or divisor of the comparison grid",
        ));
    }
    let total_micros = track.duration().as_micros() as i64;
    if total_micros % step_micros != 0 {
        return Err(bad_step(step, "does not divide the track exactly"));
    }

    let environments: Vec<EnvironmentParams> = track
        .segments
        .iter()
        .map(|s| EnvironmentParams {
            grade_fraction: s.grade_fraction,
            headwind_ms: s.headwind_ms,
            ..EnvironmentParams::default()
        })
        .collect();

    let mut times_micros = vec![0];
    let mut speeds = vec![track.initial_speed_ms];
    let mut distances = vec![0.0];

    let mut speed_ms = track.initial_speed_ms;
    let mut distance_m = 0.0;
    let mut has_moved = speed_ms > 0.0;
    let mut stopped_at = None;
    let mut stopped_at_distance_m = None;

    let mut segment_index = 0;
    let mut segment_ends_micros = track.segments[0].duration.as_micros() as i64;

    for i in 0..total_micros / step_micros {
        let start_micros = i * step_micros;
        while start_micros >= segment_ends_micros && segment_index < track.segments.len() - 1 {
            segment_index += 1;
            segment_ends_micros += track.segments[segment_index].duration.as_micros() as i64;
        }
        let segment = &track.segments[segment_index];
        let result = step_longitudinal_dynamics(
            &LongitudinalStepInput {
                speed_ms,
                power_watts: segment.power_watts,
                rider: track.rider,
                environment: environments[segment_index],
                timestep: step,
                braking_force_n: segment.braking_force_n,
                aerodynamic_drag_multiplier: segment.aerodynamic_drag_multiplier,
            },
            config,
        )?;
        speed_ms = result.next_speed_ms;
        distance_m += result.distance_delta_m;
        let end_micros = start_micros + step_micros;

        if speed_ms > 0.0 {
            has_moved = true;
        } else if has_moved && stopped_at.is_none() {
            stopped_at = Some(Duration::from_micros(end_micros as u64));
            stopped_at_distance_m = Some(distance_m);
        }

        if end_micros % grid_micros == 0 {
            times_micros.push(end_micros);
            speeds.push(speed_ms);
            distances.push(distance_m);
        }
    }

    Ok(TrackTrajectory {
        times_micros,
        speed_ms: speeds,
        distance_m: distances,
        step,
        sample_interval_micros: step_micros.max(grid_micros),
        stopped_at,
        stopped_at_distance_m,
    })
}

fn bad_step(step: Duration, message: &'static str) -> PhysicsError {
    PhysicsError::InvalidArgument {
        name: "step",
        value: format!("{step:?}"),
        message,
    }
}

/// How far one candidate track drifted from the reference.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackError {
    /// The track's name.
    pub name: &'static str,
    /// Absolute speed difference at the final sample, in m/s.
    pub final_speed_error_ms: f64,
    /// Absolute distance difference at the final sample, in metres.
    pub final_distance_error_m: f64,
    /// Largest absolute speed difference, in m/s.
    pub max_speed_error_ms: f64,
    /// Largest absolute distance difference, in metres.
    pub max_distance_error_m: f64,
    /// Difference in stopping time, if both stopped.
    pub stopping_time_error: Option<Duration>,
    /// Difference in stopping distance, in metres, if both stopped.
    pub stopping_distance_error_m: Option<f64>,
}

/// One scenario's error at one candidate step.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioError {
    /// The scenario's name.
    pub scenario: &'static str,
    /// The candidate step.
    pub step: Duration,
    /// Per-track errors, in scenario order.
    pub tracks: Vec<TrackError>,
    /// Largest error in the gap between two riders, for a two-rider scenario.
    pub max_gap_error_m: Option<f64>,
}

impl ScenarioError {
    /// The largest speed error over every track.
    pub fn max_speed_error_ms(&self) -> f64 {
        dart_reduce_max(self.tracks.iter().map(|t| t.max_speed_error_ms))
    }

    /// The largest distance error over every track.
    pub fn max_distance_error_m(&self) -> f64 {
        dart_reduce_max(self.tracks.iter().map(|t| t.max_distance_error_m))
    }
}

/// `reduce((a, b) => a > b ? a : b)`, the prototype's maximum.
pub(crate) fn dart_reduce_max(values: impl Iterator<Item = f64>) -> f64 {
    values
        .reduce(|a, b| if a > b { a } else { b })
        .expect("at least one value")
}

/// Runs `scenario` at `step` and at the finer `reference`, and reports how far apart they ended.
pub fn compare_scenario(
    scenario: &ConvergenceScenario,
    step: Duration,
    reference: Duration,
    config: &LongitudinalDynamicsConfig,
    grid: Duration,
) -> Result<ScenarioError, PhysicsError> {
    scenario
        .assert_well_formed(&[step, reference, grid])
        .map_err(PhysicsError::Solver)?;
    let candidates = scenario
        .tracks
        .iter()
        .map(|t| run_track(t, step, config, grid))
        .collect::<Result<Vec<_>, _>>()?;
    let references = scenario
        .tracks
        .iter()
        .map(|t| run_track(t, reference, config, grid))
        .collect::<Result<Vec<_>, _>>()?;

    let tracks = scenario
        .tracks
        .iter()
        .zip(candidates.iter().zip(&references))
        .map(|(t, (c, r))| compare_track(t.name, c, r))
        .collect();

    let max_gap_error_m = (scenario.tracks.len() == 2).then(|| {
        let mut worst = 0.0;
        for i in 0..candidates[0].times_micros.len() {
            let r = references[0].index_at_micros(candidates[0].times_micros[i]);
            let candidate_gap = candidates[0].distance_m[i] - candidates[1].distance_m[i];
            let reference_gap = references[0].distance_m[r] - references[1].distance_m[r];
            let error = (candidate_gap - reference_gap).abs();
            if error > worst {
                worst = error;
            }
        }
        worst
    });

    Ok(ScenarioError {
        scenario: scenario.name,
        step,
        tracks,
        max_gap_error_m,
    })
}

fn compare_track(
    name: &'static str,
    candidate: &TrackTrajectory,
    reference: &TrackTrajectory,
) -> TrackError {
    assert_eq!(
        candidate.times_micros.last(),
        reference.times_micros.last(),
        "candidate and reference ran for different durations for \"{name}\""
    );
    let mut max_speed_error_ms = 0.0;
    let mut max_distance_error_m = 0.0;
    for i in 0..candidate.times_micros.len() {
        let r = reference.index_at_micros(candidate.times_micros[i]);
        let speed_error = (candidate.speed_ms[i] - reference.speed_ms[r]).abs();
        if speed_error > max_speed_error_ms {
            max_speed_error_ms = speed_error;
        }
        let distance_error = (candidate.distance_m[i] - reference.distance_m[r]).abs();
        if distance_error > max_distance_error_m {
            max_distance_error_m = distance_error;
        }
    }
    TrackError {
        name,
        final_speed_error_ms: (candidate.final_speed_ms() - reference.final_speed_ms()).abs(),
        final_distance_error_m: (candidate.final_distance_m() - reference.final_distance_m()).abs(),
        max_speed_error_ms,
        max_distance_error_m,
        stopping_time_error: match (candidate.stopped_at, reference.stopped_at) {
            (Some(c), Some(r)) => Some(c.abs_diff(r)),
            _ => None,
        },
        stopping_distance_error_m: match (
            candidate.stopped_at_distance_m,
            reference.stopped_at_distance_m,
        ) {
            (Some(c), Some(r)) => Some((c - r).abs()),
            _ => None,
        },
    }
}
