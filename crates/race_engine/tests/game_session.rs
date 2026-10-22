//! Game lifecycle acceptance, independent of empirical world-model validity.
use race_engine::{game_session::*, mixed::*, RiderId};
use std::{sync::OnceLock, time::Duration};
fn session() -> GameSession {
    static INITIAL: OnceLock<GameSession> = OnceLock::new();
    INITIAL
        .get_or_init(|| GameSession::new(MixedOptions::default()).unwrap())
        .clone()
}
fn nominal_end() -> GameSession {
    static END: OnceLock<GameSession> = OnceLock::new();
    END.get_or_init(|| {
        let mut s = session();
        while s.workout_time() < RACE_END {
            s.step(s.target_w(), 1, None).unwrap();
        }
        s
    })
    .clone()
}
#[test]
fn warmup_has_no_field_and_actual_reserve_carries_into_a_single_initial_grid() {
    let mut s = session();
    while s.workout_time() < Duration::from_secs(599) {
        s.step(0.0, 1, None).unwrap();
        assert!(s.race_state().is_none());
    }
    for _ in 0..20 {
        s.step(600.0, 1, None).unwrap();
    }
    assert_eq!(s.phase(), Phase::Race);
    assert_eq!(s.workout_time(), RACE_START);
    // Independently: (600-300) W * 1 s = 300 J spent in the last warm-up second.
    assert!((s.reserve_j() - 19700.0).abs() < 1e-7);
    let state = s.race_state().unwrap();
    assert_eq!(state.simulation_time(), RACE_START);
    assert_eq!(
        state
            .rider_state(&RiderId::new(PLAYER).unwrap())
            .unwrap()
            .w_prime_balance_j(),
        Some(19700.0)
    );
    for r in state.riders().iter().filter(|r| r.id().as_str() != PLAYER) {
        assert_eq!(r.w_prime_balance_j(), Some(20000.0));
    }
    assert_eq!(
        state
            .rider_state(&RiderId::new("pacer").unwrap())
            .unwrap()
            .distance_m(),
        124.0
    );
    assert_eq!(
        s.events()
            .iter()
            .filter(|e| e.kind == "RaceStarted")
            .count(),
        1
    );
}
#[test]
fn natural_result_is_frozen_and_cooldown_changes_only_player_work() {
    let mut a = nominal_end();
    let mut b = a.clone();
    let state = a.race_state().unwrap().clone();
    let result = a.outcome().unwrap().clone();
    assert_eq!(a.phase(), Phase::CoolDown);
    assert_eq!(result.reason(), EndReason::Deadline);
    assert_eq!(result.verdict(), Verdict::Success);
    while a.phase() != Phase::Finished {
        a.step(0.0, 1, None).unwrap();
        b.step(600.0, 2, Some("pacer")).unwrap();
    }
    assert_eq!(a.race_state(), Some(&state));
    assert_eq!(b.race_state(), Some(&state));
    assert_eq!(a.outcome(), Some(&result));
    assert_eq!(b.outcome(), Some(&result));
    assert_ne!(a.reserve_j(), b.reserve_j());
    assert_eq!(a.active_time(), WORKOUT_END);
    assert_eq!(a.completion(), Completion::Completed);
    assert!(!a.finish());
    assert!(!a.skip_to_cooldown());
    assert!(a.step(200.0, 1, None).is_err());
}
#[test]
fn pause_skip_and_abandon_never_manufacture_a_completed_race() {
    let mut s = session();
    assert!(s.set_paused(true));
    assert!(!s.set_paused(true));
    assert!(s.step(200.0, 1, None).is_err());
    assert_eq!(s.active_time(), Duration::ZERO);
    assert!(s.skip_to_cooldown());
    assert_eq!(s.phase(), Phase::CoolDown);
    assert!(s.paused());
    assert!(s.race_state().is_none());
    assert_eq!(s.outcome().unwrap().verdict(), Verdict::Incomplete);
    assert_eq!(s.completion(), Completion::Skipped);
    assert!(!s.skip_to_cooldown());
    assert!(s.set_paused(false));
    while s.phase() != Phase::Finished {
        s.step(s.target_w(), 1, None).unwrap();
    }
    assert_eq!(s.active_time(), Duration::from_secs(300));
    assert_eq!(s.workout_time(), WORKOUT_END);
    assert_eq!(s.completion(), Completion::Skipped);
    let mut early = session();
    early.step(204.0, 1, None).unwrap();
    assert!(early.finish());
    assert_eq!(early.completion(), Completion::Abandoned);
    assert_eq!(early.outcome().unwrap().reason(), EndReason::FinishEarly);
    let mut after_race = nominal_end();
    let result = after_race.outcome().cloned();
    assert!(after_race.finish());
    assert_eq!(after_race.outcome(), result.as_ref());
    assert_eq!(after_race.completion(), Completion::Abandoned);
}
#[test]
fn missed_contribution_can_lose_the_provisional_team_objective_without_rescue() {
    let mut s = session();
    let config = s.template().config().clone();
    while s.workout_time() < RACE_END {
        s.step(if s.block() == 2 { 204.0 } else { s.target_w() }, 1, None)
            .unwrap();
    }
    assert_eq!(s.outcome().unwrap().verdict(), Verdict::Failure);
    assert!(s.outcome().unwrap().represented().is_empty());
    assert!(s.outcome().unwrap().player_front_gap_m().unwrap() > 50.0);
    assert_eq!(s.template().config(), &config);
    assert_eq!(s.phase(), Phase::CoolDown);
    assert_eq!(s.completion(), Completion::InProgress);
}
#[test]
fn competitive_execution_is_the_same_kernel_plan_as_headless_execution() {
    let mut s = session();
    let mut direct = s.template().clone();
    while s.workout_time() < RACE_START {
        s.step(s.target_w(), 1, None).unwrap();
    }
    assert_eq!(s.race_state(), Some(direct.state()));
    for _ in 0..1200 {
        let watts = s.target_w();
        s.step(watts, 1, None).unwrap();
        direct.step(watts, 1, None).unwrap();
        assert_eq!(s.race_state(), Some(direct.state()));
    }
    let mut played = s.template().clone();
    played.step(204.0, 1, None).unwrap();
    assert!(GameSession::from_template(played).is_err());
}
#[test]
fn invalid_bus_controls_are_atomic_and_samples_do_not_include_opponents() {
    let mut s = session();
    for (w, slot, wheel) in [
        (f64::NAN, 1, None),
        (200.0, 3, None),
        (200.0, 1, Some("unknown")),
        (200.0, 1, Some(PLAYER)),
    ] {
        assert!(s.step(w, slot, wheel).is_err());
        assert_eq!(s.active_time(), Duration::ZERO);
        assert_eq!(s.reserve_j(), 20000.0);
    }
    let sample = s.step(200.0, 1, None).unwrap();
    assert_eq!(sample.phase, Phase::WarmUp);
    assert!(sample.race.is_none());
}
