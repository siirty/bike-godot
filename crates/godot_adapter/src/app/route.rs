//! The route a workout becomes, as data for the profile strip and the 3D road.

use session_director::{
    segment_target_watts, GradeForPowerSynthesis, Rgb, RiderProfile, SynthesizedTerrain,
    TerrainSource, TerrainSynthesisStrategy,
};
use workout_import::WorkoutPlan;

/// Fixed so the same workout is always the same route ("the workout constrains the route, not the
/// result"). A per-workout persisted seed is future scope, as in the prototype.
pub const TERRAIN_SEED: u64 = 0;

/// Resolution of the elevation table, m.
pub const ELEVATION_STEP_M: f64 = 5.0;

/// One workout block on the route.
#[derive(Debug, Clone, PartialEq)]
pub struct Phase {
    /// Where it starts, m.
    pub start_m: f64,
    /// Where it ends, m.
    pub end_m: f64,
    /// `250 W · +3.2%`: the workout target and the grade. Never a speed: speed is the trainer's.
    pub label: String,
    /// Zone color.
    pub color: Rgb,
    /// `Threshold (91–105% FTP)`.
    pub zone_label: String,
}

/// The synthesized route.
#[derive(Debug, Clone)]
pub struct Route {
    /// The terrain the ride writes to the trainer.
    pub terrain: SynthesizedTerrain,
    /// Length, m.
    pub length_m: f64,
    /// Elevation every [`ELEVATION_STEP_M`] from 0, m above the start.
    pub elevations: Vec<f64>,
    /// The workout blocks along it.
    pub phases: Vec<Phase>,
    /// Distinct zones, in first-appearance order: (color, label).
    pub legend: Vec<(Rgb, String)>,
}

/// `flat`, or a signed percent with one decimal.
pub fn grade_label(grade: f64) -> String {
    let pct = grade * 100.0;
    if pct.abs() < 0.05 {
        "flat".to_owned()
    } else {
        format!("{}{pct:.1}%", if pct >= 0.0 { "+" } else { "" })
    }
}

/// Integrates grade over distance at the midpoint of each step.
pub fn elevation_table(terrain: &dyn TerrainSource, length_m: f64) -> Vec<f64> {
    let steps = (length_m / ELEVATION_STEP_M).ceil() as usize;
    let mut out = Vec::with_capacity(steps + 1);
    let mut h = 0.0;
    out.push(h);
    for i in 1..=steps {
        h += terrain.grade_at_distance((i as f64 - 0.5) * ELEVATION_STEP_M) * ELEVATION_STEP_M;
        out.push(h);
    }
    out
}

impl Route {
    /// The route `plan` becomes for `rider`.
    pub fn synthesize(plan: &WorkoutPlan, rider: &RiderProfile) -> Result<Self, String> {
        let terrain = GradeForPowerSynthesis
            .synthesize(plan, rider, TERRAIN_SEED)
            .map_err(|e| format!("Couldn't build a route for this workout: {e}"))?;
        let length_m = terrain.length_m().unwrap_or(0.0);
        let mut legend: Vec<(Rgb, String)> = Vec::new();
        let phases = terrain
            .segment_ranges()
            .iter()
            .map(|r| {
                let power = segment_target_watts(&r.segment, rider.ftp_watts);
                let grade =
                    terrain.grade_at_distance((r.start_distance_m + r.end_distance_m) / 2.0);
                let zone = rider.zone_for_watts(power);
                if !legend.iter().any(|(c, _)| *c == zone.color) {
                    legend.push((zone.color, zone.display_label()));
                }
                Phase {
                    start_m: r.start_distance_m,
                    end_m: r.end_distance_m,
                    label: format!("{} W · {}", power.round() as i64, grade_label(grade)),
                    color: zone.color,
                    zone_label: zone.display_label(),
                }
            })
            .collect();
        Ok(Self {
            elevations: elevation_table(&terrain, length_m),
            terrain,
            length_m,
            phases,
            legend,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use race_physics::RiderPhysicalParams;
    use std::time::Duration;
    use workout_import::{PowerTarget, WorkoutSegment};

    #[test]
    fn harder_blocks_climb_and_the_table_integrates_grade() {
        let plan = WorkoutPlan {
            segments: vec![
                WorkoutSegment::SteadyState {
                    duration: Duration::from_secs(300),
                    target_power: PowerTarget::PercentFtp(0.6),
                    cadence: None,
                },
                WorkoutSegment::SteadyState {
                    duration: Duration::from_secs(300),
                    target_power: PowerTarget::PercentFtp(1.05),
                    cadence: None,
                },
            ],
            ..WorkoutPlan::default()
        };
        let route = Route::synthesize(
            &plan,
            &RiderProfile::new(250.0, RiderPhysicalParams::new(80.0)),
        )
        .unwrap();
        assert_eq!(route.phases.len(), 2, "{:?}", route.phases);
        let (easy, hard) = (&route.phases[0], &route.phases[1]);
        let grade_mid = |p: &Phase| route.terrain.grade_at_distance((p.start_m + p.end_m) / 2.0);
        assert!(grade_mid(hard) > grade_mid(easy), "harder is never flatter");
        assert_ne!(easy.color, hard.color);
        assert_eq!(route.legend.len(), 2);
        assert!(easy.label.starts_with("150 W · "), "{}", easy.label);
        let n = route.elevations.len();
        assert_eq!(n, (route.length_m / ELEVATION_STEP_M).ceil() as usize + 1);
        assert!(
            route.elevations[n - 1] > route.elevations[(easy.end_m / ELEVATION_STEP_M) as usize]
        );
    }

    #[test]
    fn grade_labels_read_like_the_prototype() {
        assert_eq!(grade_label(0.0004), "flat");
        assert_eq!(grade_label(0.032), "+3.2%");
        assert_eq!(grade_label(-0.015), "-1.5%");
    }
}
