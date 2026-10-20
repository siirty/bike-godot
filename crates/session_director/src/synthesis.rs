use race_physics::{
    solve_grade_for_power, solve_speed_for_power, EnvironmentParams, PhysicsError, SolverOptions,
};
use workout_import::{WorkoutPlan, WorkoutSegment};

use crate::{GradePoint, RiderProfile, SegmentDistanceRange, SynthesizedTerrain};

/// Why synthesis failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SynthesisError {
    /// The plan has no segment of positive duration.
    #[error("a workout plan must have at least one segment of positive duration to synthesize terrain from")]
    NoDistance,
    /// A target was not reachable inside the solver's bracket.
    #[error(transparent)]
    Physics(#[from] PhysicsError),
    /// The produced terrain broke its own invariants (a bug).
    #[error("synthesized terrain is invalid: {0}")]
    InvalidTerrain(String),
}

/// A deterministic, frozen synthesis from a workout and rider into a route. Pre-ride only: the
/// grade is computed once and never adapted live to enforce watts.
pub trait TerrainSynthesisStrategy {
    /// Synthesizes the route. `seed` is the only permitted source of variety.
    fn synthesize(
        &self,
        plan: &WorkoutPlan,
        rider: &RiderProfile,
        seed: u64,
    ) -> Result<SynthesizedTerrain, SynthesisError>;
}

const GRADE_AT_FTP: f64 = 0.045;
const GRADE_AT_MAX_EFFORT: f64 = 0.12;
const EFFORT_CAP: f64 = 1.6;
const FREE_RIDE_SPEED_MS: f64 = 5.0;

/// The default strategy: grade anchored to each block's effort (harder is always steeper), speed
/// then solved from physics at that grade and power.
///
/// A ramp is sub-sampled (one point per ~2 s, 2–60 slices) so the terrain follows the power ramp;
/// every other block holds one grade. A free ride rides the coast-down descent that holds 5 m/s
/// at 0 W.
///
/// The seed is accepted but currently unused: the prototype's seeded jitter was removed because
/// it broke the effort→steepness guarantee, and variety is planned to come from race dynamics.
#[derive(Debug, Clone, Copy, Default)]
pub struct GradeForPowerSynthesis;

#[derive(Debug, Clone, Copy)]
struct Point {
    grade: f64,
    speed_ms: f64,
}

impl TerrainSynthesisStrategy for GradeForPowerSynthesis {
    fn synthesize(
        &self,
        plan: &WorkoutPlan,
        rider: &RiderProfile,
        _seed: u64,
    ) -> Result<SynthesizedTerrain, SynthesisError> {
        let env = EnvironmentParams::default();
        let mut samples: Vec<GradePoint> = Vec::new();
        let mut ranges = Vec::new();
        let mut distance_m = 0.0;

        for (segment_index, segment) in plan.segments.iter().enumerate() {
            let duration_s = segment.duration().as_micros() as f64 / 1e6;
            if duration_s <= 0.0 {
                continue;
            }
            let points: Vec<Point> = if let WorkoutSegment::Ramp {
                start_power,
                end_power,
                ..
            } = segment
            {
                let start = start_power.resolve_watts(rider.ftp_watts);
                let end = end_power.resolve_watts(rider.ftp_watts);
                let slices = ((duration_s / 2.0).round() as i64).clamp(2, 60);
                (0..=slices)
                    .map(|i| {
                        point_at(
                            start + (end - start) * (i as f64 / slices as f64),
                            rider,
                            &env,
                        )
                    })
                    .collect::<Result<_, _>>()?
            } else {
                vec![point_at(
                    segment_target_watts(segment, rider.ftp_watts),
                    rider,
                    &env,
                )?]
            };

            // The grade at a block boundary belongs to the block that starts there.
            if samples.last().is_some_and(|s| s.distance_m == distance_m) {
                samples.pop();
            }

            let n = points.len();
            let dt = duration_s / (n.saturating_sub(1)).max(1) as f64;
            let mut cursor = distance_m;
            for p in &points {
                if samples.last().is_none_or(|s| s.distance_m < cursor) {
                    samples.push(GradePoint {
                        distance_m: cursor,
                        grade_fraction: p.grade,
                    });
                }
                cursor += p.speed_ms * dt;
            }
            let end_m = cursor;
            let last_grade = points[n - 1].grade;
            if samples.last().is_some_and(|s| s.distance_m < end_m) {
                samples.push(GradePoint {
                    distance_m: end_m,
                    grade_fraction: last_grade,
                });
            }

            ranges.push(SegmentDistanceRange {
                segment_index,
                segment: segment.clone(),
                start_distance_m: distance_m,
                end_distance_m: end_m,
            });
            distance_m = end_m;
        }

        if samples.is_empty() {
            return Err(SynthesisError::NoDistance);
        }
        let mut deduped: Vec<GradePoint> = Vec::with_capacity(samples.len());
        for s in samples {
            if deduped.last().is_none_or(|d| s.distance_m > d.distance_m) {
                deduped.push(s);
            }
        }
        SynthesizedTerrain::new(deduped, ranges).map_err(SynthesisError::InvalidTerrain)
    }
}

fn point_at(
    watts: f64,
    rider: &RiderProfile,
    env: &EnvironmentParams,
) -> Result<Point, PhysicsError> {
    if watts == 0.0 {
        return Ok(Point {
            grade: solve_grade_for_power(
                0.0,
                FREE_RIDE_SPEED_MS,
                &rider.physical,
                env,
                SolverOptions::default(),
            )?,
            speed_ms: FREE_RIDE_SPEED_MS,
        });
    }
    let effort = (watts / rider.ftp_watts).clamp(0.0, EFFORT_CAP);
    let grade = grade_for_effort(effort);
    let speed_ms = solve_speed_for_power(
        watts,
        &rider.physical,
        &EnvironmentParams {
            grade_fraction: grade,
            ..*env
        },
        SolverOptions::default(),
    )?;
    Ok(Point { grade, speed_ms })
}

/// The grade for an effort (power / FTP): 0 at rest, linear to 4.5 % at FTP, then on to 12 % at
/// 1.6 × FTP, capped. Non-decreasing, so a harder block is never a flatter one.
pub fn grade_for_effort(effort: f64) -> f64 {
    if effort <= 0.0 {
        return 0.0;
    }
    let e = effort.clamp(0.0, EFFORT_CAP);
    let linear = if e <= 1.0 {
        GRADE_AT_FTP * e / 1.0
    } else {
        GRADE_AT_FTP + (GRADE_AT_MAX_EFFORT - GRADE_AT_FTP) * ((e - 1.0) / (EFFORT_CAP - 1.0))
    };
    linear.clamp(0.0, GRADE_AT_MAX_EFFORT)
}

/// A segment's nominal target watts: steady-state target, ramp end, interval on-start, free ride 0.
pub fn segment_target_watts(segment: &WorkoutSegment, ftp_watts: f64) -> f64 {
    match segment {
        WorkoutSegment::SteadyState { target_power, .. } => target_power.resolve_watts(ftp_watts),
        WorkoutSegment::Ramp { end_power, .. } => end_power.resolve_watts(ftp_watts),
        WorkoutSegment::IntervalRepeat(r) => r.on_start_power().resolve_watts(ftp_watts),
        WorkoutSegment::FreeRide { .. } => 0.0,
    }
}
