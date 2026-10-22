//! Immutable engineering workout and essential public context; no live state or I/O.
use crate::situations::{KnownRider, KnownStrength, PlayerKnowledge, PlayerObjective};
use std::time::Duration;
use workout_import::{PowerTarget, WorkoutPlan, WorkoutSegment};

/// Names of the eight consecutive workout blocks.
pub const BLOCK_NAMES: [&str; 8] = [
    "Warm up",
    "Sheltered endurance",
    "Chase contribution",
    "Let others work",
    "Steady contribution",
    "Short response",
    "Settle and observe",
    "Cool down",
];

/// Engineering example, not an individual training prescription.
pub fn workout() -> WorkoutPlan {
    let segments = [
        (600, 0.45, 0.68),
        (900, 0.68, 0.68),
        (180, 1.10, 1.10),
        (600, 0.68, 0.68),
        (360, 0.82, 0.82),
        (60, 1.20, 1.20),
        (600, 0.68, 0.68),
        (300, 0.55, 0.35),
    ]
    .into_iter()
    .map(|(seconds, start, end)| {
        let duration = Duration::from_secs(seconds);
        if start == end {
            WorkoutSegment::SteadyState {
                duration,
                target_power: PowerTarget::PercentFtp(start),
                cadence: None,
            }
        } else {
            WorkoutSegment::Ramp {
                duration,
                start_power: PowerTarget::PercentFtp(start),
                end_power: PowerTarget::PercentFtp(end),
                cadence: None,
            }
        }
    })
    .collect();
    WorkoutPlan {
        name: Some("Mixed race: steady participation and decisive contributions".into()),
        segments,
        ..WorkoutPlan::default()
    }
}

/// Objective and qualitative roster remain available without revealing private diagnostics.
pub fn knowledge() -> PlayerKnowledge {
    let riders = [
        ("human", "You", Some("Ember"), KnownStrength::AllRound),
        ("alex", "Alex", Some("Ember"), KnownStrength::Sustained),
        ("mara", "Mara", Some("Ember"), KnownStrength::Sprint),
        ("pacer", "Jo", Some("Blue"), KnownStrength::AllRound),
        ("rival", "Rowan", Some("Blue"), KnownStrength::Sustained),
        ("independent", "Casey", None, KnownStrength::Unknown),
    ]
    .into_iter()
    .map(|(id, name, team, strength)| {
        KnownRider::new(id.into(), name.into(), team.map(str::to_owned), strength).unwrap()
    })
    .collect();
    PlayerKnowledge::new("human".into(),PlayerObjective::SupportTeam,"Support Ember's available option. Alex attacks; Mara is your finishing option. Blue riders Jo and Rowan are opponents; Casey is unaffiliated. The rolling route has a later rise. This timed exercise has no contested finish.".into(),riders).unwrap()
}
