//! Planning properties derived from the specification ("harder blocks ride steeper", "terrain is
//! frozen and deterministic"), independent of recorded vectors.

use std::time::Duration;

use race_physics::RiderPhysicalParams;
use session_director::{
    grade_for_effort, zone_for_effort, FlatTerrain, GradeForPowerSynthesis, RiderProfile,
    SynthesisError, TerrainSource, TerrainSynthesisStrategy,
};
use workout_import::{PowerTarget, WorkoutPlan, WorkoutSegment};

fn rider() -> RiderProfile {
    RiderProfile::new(250.0, RiderPhysicalParams::new(78.0))
}

fn steady(seconds: u64, fraction: f64) -> WorkoutSegment {
    WorkoutSegment::SteadyState {
        duration: Duration::from_secs(seconds),
        target_power: PowerTarget::PercentFtp(fraction),
        cadence: None,
    }
}

fn plan(segments: Vec<WorkoutSegment>) -> WorkoutPlan {
    WorkoutPlan {
        name: Some("p".into()),
        author: None,
        description: None,
        segments,
    }
}

#[test]
fn a_harder_block_is_never_flatter() {
    let mut previous = f64::NEG_INFINITY;
    for i in 0..=200 {
        let g = grade_for_effort(i as f64 / 100.0);
        assert!(g >= previous);
        assert!((0.0..=0.12).contains(&g));
        previous = g;
    }
    assert_eq!(grade_for_effort(1.0), 0.045);
    assert_eq!(grade_for_effort(1.6), 0.12);
    assert_eq!(grade_for_effort(5.0), 0.12);

    let t = GradeForPowerSynthesis
        .synthesize(
            &plan(vec![steady(60, 0.6), steady(60, 1.2), steady(60, 0.6)]),
            &rider(),
            0,
        )
        .unwrap();
    let r = t.segment_ranges();
    let mid = |i: usize| t.grade_at_distance((r[i].start_distance_m + r[i].end_distance_m) / 2.0);
    assert!(mid(1) > mid(0));
    assert_eq!(mid(0), mid(2));
}

#[test]
fn ranges_tile_the_course_and_zero_duration_blocks_take_no_distance() {
    let t = GradeForPowerSynthesis
        .synthesize(
            &plan(vec![steady(30, 0.7), steady(0, 1.5), steady(45, 0.9)]),
            &rider(),
            0,
        )
        .unwrap();
    let r = t.segment_ranges();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].segment_index, 0);
    assert_eq!(r[1].segment_index, 2);
    assert_eq!(r[0].start_distance_m, 0.0);
    assert_eq!(r[0].end_distance_m, r[1].start_distance_m);
    assert_eq!(t.length_m(), Some(r[1].end_distance_m));
    assert_eq!(t.segment_range_at(1.0).unwrap().segment_index, 0);
}

#[test]
fn free_ride_is_a_coast_down_descent_and_an_empty_plan_is_refused() {
    let free = WorkoutSegment::FreeRide {
        duration: Duration::from_secs(60),
        cadence: None,
    };
    let t = GradeForPowerSynthesis
        .synthesize(&plan(vec![free]), &rider(), 0)
        .unwrap();
    assert!(t.grade_at_distance(10.0) < 0.0);
    assert_eq!(t.length_m(), Some(300.0), "5 m/s for 60 s");

    assert_eq!(
        GradeForPowerSynthesis.synthesize(&plan(vec![]), &rider(), 0),
        Err(SynthesisError::NoDistance)
    );
    assert_eq!(
        GradeForPowerSynthesis.synthesize(&plan(vec![steady(0, 1.0)]), &rider(), 0),
        Err(SynthesisError::NoDistance)
    );
}

#[test]
fn synthesis_is_deterministic_and_lookups_hold_outside_the_course() {
    let p = plan(vec![steady(120, 0.8), steady(60, 1.1)]);
    let a = GradeForPowerSynthesis.synthesize(&p, &rider(), 1).unwrap();
    let b = GradeForPowerSynthesis.synthesize(&p, &rider(), 1).unwrap();
    assert_eq!(a, b);
    let first = a.samples()[0].grade_fraction;
    let last = a.samples().last().unwrap().grade_fraction;
    assert_eq!(a.grade_at_distance(-5.0), first);
    assert_eq!(a.grade_at_distance(f64::NAN), first);
    assert_eq!(a.grade_at_distance(1e9), last);
    assert_eq!(
        FlatTerrain {
            grade_fraction: 0.02
        }
        .grade_at_distance(123.0),
        0.02
    );
}

#[test]
fn zones_are_lower_inclusive_and_resolve_against_ftp() {
    assert_eq!(zone_for_effort(0.55).name, "Z2");
    assert_eq!(zone_for_effort(0.5499).name, "Z1");
    assert_eq!(zone_for_effort(1.12).name, "Z5");
    assert_eq!(zone_for_effort(f64::NAN).name, "Z1");
    assert_eq!(rider().zone_for_watts(245.0).name, "Z4");
    assert_eq!(
        RiderProfile::new(0.0, RiderPhysicalParams::new(70.0))
            .zone_for_watts(300.0)
            .name,
        "Z1"
    );
}
