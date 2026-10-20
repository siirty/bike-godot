//! Behavioral specifications, not regenerated output snapshots.
use race_engine::harness::{run_race, TacticalScenario};
use race_engine::tactics::*;
use race_engine::*;
use race_physics::{LongitudinalDynamicsConfig, PhysiologyParams, RiderPhysicalParams};
use std::time::Duration;

fn id(s: &str) -> RiderId {
    RiderId::new(s).unwrap()
}
fn fixture(
    gap: f64,
    reserve: f64,
    time: u64,
    finish: Option<f64>,
) -> (RaceSimulationConfig, RaceState) {
    let mut spec = RaceSimulationConfigSpec::new(
        Duration::from_millis(50),
        ["a", "b"]
            .into_iter()
            .map(|name| {
                RiderDefinition::new(id(name), RiderPhysicalParams::new(80.0))
                    .unwrap()
                    .with_physiology(PhysiologyParams::new(250.0, 20_000.0, 1000.0).unwrap())
            })
            .collect(),
        SimulationTerrain::new(
            vec![TerrainSample::new(0.0, 0.0).unwrap()],
            TerrainInterpolation::Linear,
            BeyondEndBehavior::Flat,
            finish,
        )
        .unwrap(),
        LongitudinalDynamicsConfig::default(),
        Some(BasicDraftingConfig::new(6.0, vec![0.62, 0.85]).unwrap()),
    );
    spec.lateral_slot_count = 3;
    let config = RaceSimulationConfig::new(spec).unwrap();
    let state = RaceState::new(
        Duration::from_millis(time),
        Pcg32::seeded(42, 1).state(),
        vec![
            RiderState::new(id("a"), 100.0, 9.0, 0, None)
                .unwrap()
                .with_w_prime_balance(reserve)
                .unwrap(),
            RiderState::new(id("b"), 100.0 + gap, 10.0, 1, None)
                .unwrap()
                .with_w_prime_balance(20_000.0)
                .unwrap(),
        ],
    )
    .unwrap();
    (config, state)
}
fn controller(aggression: f64) -> TacticalController {
    TacticalController::new(
        id("a"),
        TacticsConfig::default(),
        TacticalProfile::new(aggression, 1.0, 0.5).unwrap(),
    )
}

#[test]
fn follows_a_close_wheel_and_chases_a_break_without_changing_capability() {
    let (config, close) = fixture(4.0, 20_000.0, 0, None);
    let mut policy = controller(0.0);
    let follow = policy
        .input(&config, &close, TacticalRole::Neutral)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Follow));
    assert_eq!(follow.draft_target_id(), Some(&id("b")));
    assert_eq!(follow.lateral_slot(), Some(1));
    // Independent arithmetic: 0.9*250 + 8*(4-4) + 35*(10-9) = 260 W.
    assert_eq!(follow.power_watts(), 260.0);
    let (config, distant) = fixture(25.0, 20_000.0, 0, None);
    let original = config.clone();
    let mut policy = controller(0.0);
    let chase = policy
        .input(&config, &distant, TacticalRole::Chaser)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Chase));
    assert_eq!(chase.power_watts(), 312.5); // 1.25 CP chase ceiling
    assert_eq!(config, original);
}
#[test]
fn reserve_recovery_overrides_roles_and_commitment_with_hysteresis() {
    let (config, fresh) = fixture(-4.0, 20_000.0, 0, None);
    let mut policy = controller(1.0);
    policy
        .input(&config, &fresh, TacticalRole::Instigator)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Attack));
    let (_, tired) = fixture(-4.0, 1000.0, 50, None);
    let recovery = policy
        .input(&config, &tired, TacticalRole::Instigator)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Recover));
    assert_eq!(recovery.power_watts(), 162.5); // 65% of fixed 250 W CP
    let (_, partial) = fixture(-4.0, 6000.0, 1050, None);
    policy
        .input(&config, &partial, TacticalRole::Instigator)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Recover)); // still below 40%
    let (_, replenished) = fixture(-4.0, 15_000.0, 4050, None);
    policy
        .input(&config, &replenished, TacticalRole::Instigator)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Attack));
}
#[test]
fn cadence_and_commitment_hold_then_allow_a_better_action() {
    let (config, state) = fixture(4.0, 20_000.0, 0, Some(1000.0));
    let mut policy = controller(0.0);
    let original = policy
        .input(&config, &state, TacticalRole::Neutral)
        .unwrap();
    // The target remains present, but geometry now makes a finish sprint preferable.
    let near_finish = |millis| {
        RaceState::new(
            Duration::from_millis(millis),
            state.random_state(),
            vec![
                RiderState::new(id("a"), 900.0, 9.0, 0, None)
                    .unwrap()
                    .with_w_prime_balance(20_000.0)
                    .unwrap(),
                RiderState::new(id("b"), 904.0, 10.0, 1, None)
                    .unwrap()
                    .with_w_prime_balance(20_000.0)
                    .unwrap(),
            ],
        )
        .unwrap()
    };
    assert_eq!(
        policy
            .input(&config, &near_finish(500), TacticalRole::Neutral)
            .unwrap(),
        original
    );
    policy
        .input(&config, &near_finish(1000), TacticalRole::Neutral)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Follow));
    policy
        .input(&config, &near_finish(3000), TacticalRole::Neutral)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Sprint));
}
#[test]
fn observation_limits_and_finish_are_explicit_not_workout_driven() {
    let (config, far) = fixture(101.0, 20_000.0, 0, None);
    let mut policy = controller(1.0);
    policy
        .input(&config, &far, TacticalRole::Instigator)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Cruise)); // no visible attack target
    let (config, state) = fixture(4.0, 20_000.0, 0, Some(200.0));
    let mut policy = controller(1.0);
    let input = policy
        .input(&config, &state, TacticalRole::Instigator)
        .unwrap();
    assert_eq!(policy.action(), Some(TacticalAction::Sprint));
    assert_eq!(input.power_watts(), 1000.0); // finite fresh Pmax, no capability boost
}
#[test]
fn checkpoint_and_kernel_snapshot_resume_identical_actions_states_and_events() {
    let mut scenario = TacticalScenario::generate(3).unwrap();
    let sim = RaceSimulation::new(scenario.base.config.clone()).unwrap();
    let prefix = run_race(&sim, scenario.base.initial_state.clone(), 137, |s| {
        scenario.inputs_at(s)
    })
    .unwrap();
    let snapshot = encode_snapshot(sim.config(), &prefix.final_state).unwrap();
    let (_, mut b) = decode_snapshot(&snapshot).unwrap().into_parts();
    let mut a = prefix.final_state;
    let mut resumed = scenario.clone();
    resumed.controllers = scenario
        .controllers
        .iter()
        .map(|c| TacticalController::restore(&c.checkpoint()).unwrap())
        .collect();
    for _ in 0..4000 {
        let left = scenario.inputs_at(&a).unwrap();
        let right = resumed.inputs_at(&b).unwrap();
        assert_eq!(left, right);
        let left = sim.step(&a, &left, sim.config().fixed_step()).unwrap();
        let right = sim.step(&b, &right, sim.config().fixed_step()).unwrap();
        assert_eq!(left, right);
        a = left.next_state;
        b = right.next_state;
    }
}
#[test]
fn mixed_field_seed_corpus_is_reproducible_and_finishes_with_fixed_capabilities() {
    let mut actions = [0u64; 6];
    for seed in 0..8 {
        let mut scenario = TacticalScenario::generate(seed).unwrap();
        let original = scenario.base.config.clone();
        let (run, counts) = scenario.run().unwrap();
        assert_eq!(
            run.finishers().len(),
            original.riders().len(),
            "seed {seed}"
        );
        assert_eq!(scenario.base.config, original);
        for (sum, count) in actions.iter_mut().zip(counts) {
            *sum += count;
        }
        if seed == 0 {
            let (other, _) = TacticalScenario::generate(seed).unwrap().run().unwrap();
            assert_eq!(run, other);
        }
    }
    assert!(actions[TacticalAction::Chase as usize] > 0);
    assert!(actions[TacticalAction::Follow as usize] > 0);
    assert!(actions[TacticalAction::Recover as usize] > 0);
    assert!(actions[TacticalAction::Sprint as usize] > 0);
}
#[test]
fn rejects_invalid_configuration_checkpoint_and_time_without_corrupting_policy() {
    for spec in [
        TacticsSpec {
            decision_interval: Duration::ZERO,
            ..Default::default()
        },
        TacticsSpec {
            gap_gain: f64::NAN,
            ..Default::default()
        },
        TacticsSpec {
            recovery_enter: 0.5,
            recovery_exit: 0.2,
            ..Default::default()
        },
    ] {
        assert!(TacticsConfig::new(spec).is_err());
    }
    assert!(TacticalProfile::new(f64::INFINITY, 0.0, 0.0).is_err());
    let (config, state) = fixture(4.0, 20_000.0, 1000, None);
    let mut policy = controller(0.0);
    policy
        .input(&config, &state, TacticalRole::Neutral)
        .unwrap();
    let checkpoint = policy.checkpoint();
    assert!(
        TacticalController::restore(&checkpoint.replace("\"version\":1", "\"version\":99"))
            .is_err()
    );
    let (_, past) = fixture(4.0, 20_000.0, 500, None);
    assert!(policy.input(&config, &past, TacticalRole::Neutral).is_err());
    assert_eq!(policy.checkpoint(), checkpoint);
    assert!(
        TacticalController::restore(&checkpoint.replace("\"watts\":260.0", "\"watts\":-1.0"))
            .is_err()
    );
}

#[test]
fn role_bias_can_change_a_close_choice_but_cannot_invent_an_attack_target() {
    // Attack utility .25 + .45*aggression = .61 versus cruise .40;
    // close-wheel follow .65 wins neutral, but instigator raises attack to .76.
    let (config, state) = fixture(4.0, 20_000.0, 0, None);
    let profile = TacticalProfile::new(0.8, 0.0, 0.0).unwrap();
    let make = || TacticalController::new(id("a"), TacticsConfig::default(), profile);
    let mut neutral = make();
    neutral
        .input(&config, &state, TacticalRole::Neutral)
        .unwrap();
    assert_eq!(neutral.action(), Some(TacticalAction::Follow));
    let mut instigator = make();
    instigator
        .input(&config, &state, TacticalRole::Instigator)
        .unwrap();
    assert_eq!(instigator.action(), Some(TacticalAction::Attack));
    let (config, distant) = fixture(1000.0, 20_000.0, 0, None);
    let mut instigator = make();
    instigator
        .input(&config, &distant, TacticalRole::Instigator)
        .unwrap();
    assert_eq!(instigator.action(), Some(TacticalAction::Cruise));
}

#[test]
fn finished_riders_are_not_chased_and_targets_tie_break_by_canonical_id() {
    let (config, state) = fixture(10.0, 20_000.0, 0, Some(105.0));
    let mut policy = controller(0.0);
    let input = policy.input(&config, &state, TacticalRole::Chaser).unwrap();
    assert_eq!(input.draft_target_id(), None); // b already finished
    let mut scenario = TacticalScenario::generate(0).unwrap();
    let ids: Vec<_> = scenario
        .base
        .config
        .riders()
        .iter()
        .map(|r| r.id().clone())
        .collect();
    let riders = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let d = if i == 0 { 100.0 } else { 104.0 };
            let capacity = scenario
                .base
                .config
                .definition_for(id)
                .unwrap()
                .physiology()
                .unwrap()
                .w_prime_j();
            RiderState::new(id.clone(), d, 9.0, 0, None)
                .unwrap()
                .with_w_prime_balance(capacity)
                .unwrap()
        })
        .collect();
    let state = RaceState::new(
        Duration::ZERO,
        scenario.base.initial_state.random_state(),
        riders,
    )
    .unwrap();
    let policy = TacticalController::new(
        ids[0].clone(),
        TacticsConfig::default(),
        TacticalProfile::new(0.0, 0.0, 0.5).unwrap(),
    );
    scenario.controllers = vec![policy];
    let inputs = scenario.inputs_at(&state).unwrap();
    assert_eq!(
        inputs.input_for(&ids[0]).unwrap().draft_target_id(),
        Some(&ids[1])
    );
}
