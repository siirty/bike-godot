//! Public knowledge, validation and tactical context regression checks.
use session_director::situations::*;
fn rider(id: &str, team: Option<&str>) -> KnownRider {
    KnownRider::new(
        id.into(),
        id.into(),
        team.map(str::to_owned),
        KnownStrength::Unknown,
    )
    .unwrap()
}
fn knowledge() -> PlayerKnowledge {
    PlayerKnowledge::new(
        "p".into(),
        PlayerObjective::ContestFinish,
        "Known context".into(),
        vec![rider("p", Some("green")), rider("q", Some("blue"))],
    )
    .unwrap()
}
fn situation(start: u32, duration: u32, id: &str) -> LearningSituation {
    LearningSituation::new(SituationSpec {
        id: id.into(),
        start_s: start,
        duration_s: duration,
        intent: LearningIntent::Respond,
        cues: vec!["Opening gap".into()],
        responses: vec!["Follow either available wheel".into()],
        response_window_s: 2.0,
        effort: EffortEnvelope::new(200.0, 250.0, 0.8).unwrap(),
        consequence: "A late response can lose contact.".into(),
        exit: "Stay in contention".into(),
    })
    .unwrap()
}
#[test]
fn missing_player_and_duplicate_rosters_are_rejected() {
    assert!(PlayerKnowledge::new(
        "missing".into(),
        PlayerObjective::ContestFinish,
        "context".into(),
        vec![rider("p", None)]
    )
    .is_err());
    assert!(PlayerKnowledge::new(
        "p".into(),
        PlayerObjective::ContestFinish,
        "context".into(),
        vec![rider("p", None), rider("p", Some("other"))]
    )
    .is_err());
    assert!(KnownRider::new("id".into(), " ".into(), None, KnownStrength::Unknown).is_err());
}
#[test]
fn no_team_is_not_a_shared_affiliation() {
    let k = PlayerKnowledge::new(
        "p".into(),
        PlayerObjective::ContestFinish,
        "context".into(),
        vec![rider("p", None), rider("q", None)],
    )
    .unwrap();
    assert!(!k.teammates("p", "q"));
    assert!(!k.teammates("missing", "q"));
    assert!(!knowledge().teammates("p", "q"));
}
#[test]
fn sequence_rejects_gaps_overlaps_duplicates_and_missing_start() {
    for ss in [
        vec![situation(1, 10, "a")],
        vec![situation(0, 10, "a"), situation(11, 10, "b")],
        vec![situation(0, 10, "a"), situation(9, 10, "b")],
        vec![situation(0, 10, "a"), situation(10, 10, "a")],
    ] {
        assert!(LearningPlan::new(knowledge(), ss).is_err());
    }
    assert!(LearningPlan::new(
        knowledge(),
        vec![situation(0, 10, "a"), situation(10, 20, "b")]
    )
    .is_ok());
}
#[test]
fn effort_rejects_invalid_numbers_and_keeps_inclusive_bounds() {
    for (lo, hi, share) in [
        (f64::NAN, 200.0, 0.8),
        (0.0, f64::INFINITY, 0.8),
        (-1.0, 200.0, 0.8),
        (250.0, 200.0, 0.8),
        (0.0, 200.0, 1.1),
        (0.0, 200.0, f64::NAN),
    ] {
        assert!(EffortEnvelope::new(lo, hi, share).is_err());
    }
    let e = EffortEnvelope::new(200.0, 250.0, 0.8).unwrap();
    assert!(e.contains(200.0) && e.contains(250.0));
    assert!(!e.contains(f64::NAN));
}
#[test]
fn public_knowledge_serialization_contains_no_authored_answers_or_physiology() {
    let json = serde_json::to_string(&knowledge()).unwrap();
    for secret in [
        "reserve",
        "critical_power",
        "intent",
        "response_window",
        "effort",
    ] {
        assert!(!json.contains(secret));
    }
}
