//! Adversarial checks of the experiment's invariants and diagnostics.
use race_prototype::{compile, replay, workout, Mode, Options};

#[test]
fn all_zones_have_contiguous_work_and_recovery_without_resetting_state() {
    let plan = compile(Options::default()).unwrap();
    let mut zones: Vec<_> = plan.phases.iter().map(|p| p.zone).collect();
    zones.sort();
    zones.dedup();
    assert_eq!(zones, vec![1, 2, 3, 4, 5, 6, 7]);
    for pair in plan.phases.windows(2) {
        assert_eq!(pair[0].start + pair[0].duration, pair[1].start);
    }
    let run = replay(&plan, Mode::Prescribed).unwrap();
    assert_eq!(run.outcomes.len(), plan.phases.len());
    assert!(
        run.outcomes.iter().all(|o| o.passed),
        "nominal action calibration must meet its stated gates"
    );
    for w in run.samples.windows(2) {
        assert!(w[1].distance >= w[0].distance);
        for i in 0..5 {
            let d0 = w[0].distance + w[0].gaps[i];
            let d1 = w[1].distance + w[1].gaps[i];
            assert!(d1 >= d0, "rider {i} moved backwards at {}", w[1].t);
            assert!(d1 - d0 < 30.0, "rider {i} teleported at {}", w[1].t);
        }
    }
    assert!(run.finish_s.is_some());
}
#[test]
fn same_seed_reproduces_scripts_and_outcomes_and_replays_leave_plan_unchanged() {
    let plan = compile(Options::default()).unwrap();
    let other = compile(Options::default()).unwrap();
    assert_eq!(plan.script, other.script);
    assert_eq!(plan.config, other.config);
    let script = plan.script.clone();
    let config = plan.config.clone();
    let a = replay(&plan, Mode::Prescribed).unwrap();
    let b = replay(&other, Mode::Prescribed).unwrap();
    assert_eq!(a, b);
    for mode in [Mode::Under, Mode::Over, Mode::MissBridge, Mode::CoverDecoy] {
        replay(&plan, mode).unwrap();
    }
    assert_eq!(script, plan.script);
    assert_eq!(config, plan.config);
}
#[test]
fn failed_effort_has_persistent_geometry_consequences_in_following_recovery() {
    let plan = compile(Options::default()).unwrap();
    let nominal = replay(&plan, Mode::Prescribed).unwrap();
    let missed = replay(&plan, Mode::MissBridge).unwrap();
    let bridge = plan.phases.iter().position(|p| p.zone == 6).unwrap();
    assert!(missed.outcomes[bridge].exit_gap > nominal.outcomes[bridge].exit_gap + 20.0);
    assert!(missed.outcomes[bridge + 1].entry_gap > nominal.outcomes[bridge + 1].entry_gap + 20.0);
    assert!(!missed.outcomes[bridge].passed);
}
#[test]
fn following_actions_is_independent_of_the_prescribed_watts_and_can_fail_the_workout() {
    let mut plan = compile(Options::default()).unwrap();
    let before = replay(&plan, Mode::FollowAction).unwrap();
    plan.phases.iter_mut().for_each(|p| p.intensity *= 0.5);
    let after = replay(&plan, Mode::FollowAction).unwrap();
    for (a, b) in before.samples.iter().zip(&after.samples) {
        assert_eq!((a.distance, a.power, a.gaps), (b.distance, b.power, b.gaps));
        assert_eq!(a.demand, b.demand);
        assert_eq!(a.scene, b.scene);
    }
    assert_eq!(before.transitions, after.transitions);
    assert!(after.outcomes.iter().any(|o| !o.passed));
}
#[test]
fn delivery_of_prescribed_watts_is_not_sufficient_to_pass_formation_checks() {
    let plan = compile(Options::default()).unwrap();
    let run = replay(&plan, Mode::Under).unwrap();
    assert!(run
        .outcomes
        .iter()
        .any(|o| o.intensity_share > 0.8 && !o.passed && o.formation_share < 0.5));
}
#[test]
fn ignore_and_cover_decoy_comparison_reports_actual_extra_work() {
    let plan = compile(Options::default()).unwrap();
    let ignore = replay(&plan, Mode::Prescribed).unwrap();
    let cover = replay(&plan, Mode::CoverDecoy).unwrap();
    let mut extra = 0.0;
    for (i, p) in plan
        .phases
        .iter()
        .enumerate()
        .filter(|(_, p)| p.zone == 1 && p.start > 0 && p.start < 900)
    {
        extra +=
            (cover.outcomes[i].mean_power - ignore.outcomes[i].mean_power) * f64::from(p.duration);
    }
    assert!(
        extra > 1000.0,
        "covering the authored decoy should visibly cost energy"
    );
}
#[test]
fn bounded_profile_matrix_runs_and_non_finite_options_are_rejected() {
    for (ftp, mass, seed) in [(180.0, 65.0, 1), (320.0, 92.0, 7), (250.0, 110.0, 99)] {
        let plan = compile(Options { ftp, mass, seed }).unwrap();
        for mode in [Mode::Prescribed, Mode::Under, Mode::Over] {
            let run = replay(&plan, mode).unwrap();
            assert_eq!(run.outcomes.len(), workout().len());
            assert!(run.samples.iter().all(|s| s.power.is_finite()
                && s.demand.is_finite()
                && s.reserves.iter().all(|r| (0.0..=1.0).contains(r))));
        }
    }
    for ftp in [f64::NAN, f64::INFINITY, -100.0, 0.0, 1000.0] {
        assert!(compile(Options {
            ftp,
            ..Options::default()
        })
        .is_err());
    }
}
