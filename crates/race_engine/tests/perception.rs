//! Public knowledge, validation and tactical context regression checks.
use race_engine::perception::*;
use session_director::situations::*;
fn knowledge(team: &str, strength: KnownStrength, objective: PlayerObjective) -> PlayerKnowledge {
    PlayerKnowledge::new(
        "p".into(),
        objective,
        "Contest from known context".into(),
        vec![
            KnownRider::new(
                "p".into(),
                "Player".into(),
                Some("green".into()),
                KnownStrength::AllRound,
            )
            .unwrap(),
            KnownRider::new("q".into(), "Rider".into(), Some(team.into()), strength).unwrap(),
            KnownRider::new(
                "a".into(),
                "Ally".into(),
                Some("green".into()),
                KnownStrength::Sustained,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
fn view() -> PublicRaceView {
    PublicRaceView {
        riders: vec![RiderMotion {
            id: "q".into(),
            gap_m: 12.0,
            relative_speed_ms: 1.0,
        }],
        grade: 0.02,
        finish_remaining_m: Some(2000.0),
    }
}
fn read(k: &PlayerKnowledge, v: &PublicRaceView) -> MoveRead {
    read_move(k, v, ReaderSettings::default()).unwrap()
}
#[test]
fn identical_motion_changes_meaning_with_affiliation_and_objective() {
    let v = view();
    assert_eq!(
        read(
            &knowledge(
                "blue",
                KnownStrength::Sustained,
                PlayerObjective::ContestFinish
            ),
            &v
        )
        .decision,
        MoveDecision::Respond
    );
    assert_eq!(
        read(
            &knowledge(
                "green",
                KnownStrength::Sustained,
                PlayerObjective::ContestFinish
            ),
            &v
        )
        .decision,
        MoveDecision::LetOthersWork
    );
    assert_eq!(
        read(
            &knowledge(
                "green",
                KnownStrength::Sustained,
                PlayerObjective::StayWithGroup
            ),
            &v
        )
        .decision,
        MoveDecision::Respond
    );
}
#[test]
fn teammate_representation_changes_chase_responsibility() {
    let k = knowledge(
        "blue",
        KnownStrength::Sustained,
        PlayerObjective::ContestFinish,
    );
    let mut v = view();
    v.riders.push(RiderMotion {
        id: "a".into(),
        gap_m: 14.0,
        relative_speed_ms: 1.0,
    });
    assert_eq!(read(&k, &v).decision, MoveDecision::LetOthersWork);
    assert_eq!(
        read(
            &knowledge(
                "blue",
                KnownStrength::Sustained,
                PlayerObjective::StayWithGroup
            ),
            &v
        )
        .decision,
        MoveDecision::Respond
    );
}
#[test]
fn unknown_reputation_is_uncertainty_not_a_hidden_threat_label() {
    let mut v = view();
    assert_eq!(
        read(
            &knowledge(
                "blue",
                KnownStrength::Unknown,
                PlayerObjective::ContestFinish
            ),
            &v
        )
        .decision,
        MoveDecision::Monitor
    );
    v.riders[0].id = "unfamiliar".into();
    assert_eq!(
        read(
            &knowledge(
                "blue",
                KnownStrength::Sustained,
                PlayerObjective::ContestFinish
            ),
            &v
        )
        .decision,
        MoveDecision::Monitor
    );
}
#[test]
fn finish_context_changes_decision_without_a_workout_clock() {
    let k = knowledge(
        "blue",
        KnownStrength::Sprint,
        PlayerObjective::ContestFinish,
    );
    let mut v = view();
    assert_eq!(read(&k, &v).decision, MoveDecision::Monitor);
    v.finish_remaining_m = Some(100.0);
    assert_eq!(read(&k, &v).decision, MoveDecision::Sprint);
    v.finish_remaining_m = Some(-0.1);
    assert_eq!(read(&k, &v).decision, MoveDecision::Finished);
    v.finish_remaining_m = None;
    assert_ne!(read(&k, &v).decision, MoveDecision::Sprint);
}
#[test]
fn invalid_observations_and_settings_are_rejected() {
    let k = knowledge(
        "blue",
        KnownStrength::Sustained,
        PlayerObjective::ContestFinish,
    );
    let mut v = view();
    v.riders[0].gap_m = f64::NAN;
    assert!(read_move(&k, &v, ReaderSettings::default()).is_err());
    assert!(read_move(
        &k,
        &view(),
        ReaderSettings {
            visibility_m: 1.0,
            ..ReaderSettings::default()
        }
    )
    .is_err());
}
#[test]
fn irrelevant_roster_names_do_not_change_the_read() {
    let k = knowledge(
        "blue",
        KnownStrength::Sustained,
        PlayerObjective::ContestFinish,
    );
    let renamed = PlayerKnowledge::new(
        "p".into(),
        k.objective(),
        "Other wording".into(),
        k.riders()
            .iter()
            .map(|r| {
                KnownRider::new(
                    r.id().into(),
                    format!("Renamed {}", r.id()),
                    r.team().map(str::to_owned),
                    r.strength(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    assert_eq!(read(&k, &view()), read(&renamed, &view()));
}
