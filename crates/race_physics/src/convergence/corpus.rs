use std::time::Duration;

use super::scenario::{ConvergenceScenario, InputSegment as S, RiderTrack};
use crate::{PhysicsDefaults, RiderPhysicalParams};

/// A 78 kg rider on default equipment.
pub const CONVERGENCE_ALL_ROUNDER: RiderPhysicalParams = RiderPhysicalParams::new(78.0);

/// A lighter rider with a slightly better position, for the climb.
pub const CONVERGENCE_CLIMBER: RiderPhysicalParams = RiderPhysicalParams {
    total_mass_kg: 66.0,
    crr: PhysicsDefaults::CRR,
    cda_m2: 0.30,
    drive_train_efficiency: PhysicsDefaults::DRIVE_TRAIN_EFFICIENCY,
};

/// The candidate fixed steps, coarsest first. The coarse end is there to fail.
pub const CANDIDATE_STEPS: [Duration; 4] = [
    Duration::from_millis(200),
    Duration::from_millis(100),
    Duration::from_millis(50),
    Duration::from_millis(20),
];

/// The step every candidate is measured against: the same integrator at 2 ms.
pub const REFERENCE_STEP: Duration = Duration::from_millis(2);

fn track(name: &'static str, rider: RiderPhysicalParams, v0: f64, segments: Vec<S>) -> RiderTrack {
    RiderTrack {
        name,
        rider,
        segments,
        initial_speed_ms: v0,
    }
}

/// Every scenario the timestep selection is judged on. Chosen to cover the integrator's distinct
/// numerical behaviours, not to be a plausible ride.
pub fn convergence_corpus() -> Vec<ConvergenceScenario> {
    let a = CONVERGENCE_ALL_ROUNDER;
    vec![
        ConvergenceScenario {
            name: "accelerate-from-rest",
            what: "the force cap, the crossover into power-limited riding, and the steepest part \
                   of the velocity curve",
            tracks: vec![track("rider", a, 0.0, vec![S::coast(120).power(250.0)])],
        },
        ConvergenceScenario {
            name: "power-step-then-coast",
            what: "a step change in power, then deceleration under resistance alone with no rider \
                   input",
            tracks: vec![track(
                "rider",
                a,
                0.0,
                vec![S::coast(60).power(300.0), S::coast(120)],
            )],
        },
        ConvergenceScenario {
            name: "braking-moderate-descent",
            what: "braking held against an assisting grade — a balance that settles rather than \
                   stops",
            tracks: vec![track(
                "rider",
                a,
                8.0,
                vec![S::coast(60).grade(-0.06).braking(40.0)],
            )],
        },
        ConvergenceScenario {
            name: "braking-to-stop",
            what: "the zero-speed clamp: when the rider stops, how far they got, and that they \
                   stay stopped",
            tracks: vec![track(
                "rider",
                a,
                12.0,
                vec![S::coast(20).braking(250.0), S::coast(10)],
            )],
        },
        ConvergenceScenario {
            name: "sustained-climb",
            what: "gravity dominating the balance for five minutes",
            tracks: vec![track(
                "climber",
                CONVERGENCE_CLIMBER,
                4.0,
                vec![S::coast(300).power(280.0).grade(0.08)],
            )],
        },
        ConvergenceScenario {
            name: "descent-coasting",
            what: "a rider accelerating towards terminal speed on gravity alone, where aero drag \
                   is largest",
            tracks: vec![track("rider", a, 5.0, vec![S::coast(120).grade(-0.08)])],
        },
        ConvergenceScenario {
            name: "grade-transition",
            what:
                "two discontinuities in grade at constant power — the case a coarse step handles \
                   worst, because it carries the old grade across the whole step containing the \
                   change",
            tracks: vec![track(
                "rider",
                a,
                10.0,
                vec![
                    S::coast(40).power(250.0),
                    S::coast(60).power(250.0).grade(0.06),
                    S::coast(60).power(250.0).grade(-0.06),
                ],
            )],
        },
        ConvergenceScenario {
            name: "headwind-then-tailwind",
            what:
                "both signs of relative airspeed, ending with a tailwind faster than the rider — \
                   where aerodynamic force assists instead of resisting",
            tracks: vec![track(
                "rider",
                a,
                10.0,
                vec![
                    S::coast(60).power(250.0).headwind(8.0),
                    S::coast(60).power(250.0).headwind(-8.0),
                    S::coast(60).headwind(-14.0),
                ],
            )],
        },
        ConvergenceScenario {
            name: "close-gap-pair",
            what: "two nearly matched riders — one pushing, one sheltered — over a flat section \
                   and a climb; the drift measured is in the gap between them, not in either \
                   trajectory",
            tracks: vec![
                track(
                    "leader",
                    a,
                    10.0,
                    vec![
                        S::coast(150).power(260.0),
                        S::coast(150).power(260.0).grade(0.04),
                    ],
                ),
                track(
                    "follower",
                    a,
                    10.0,
                    vec![
                        S::coast(150).power(250.0).drag_multiplier(0.93),
                        S::coast(150).power(250.0).grade(0.04).drag_multiplier(0.93),
                    ],
                ),
            ],
        },
        ConvergenceScenario {
            name: "long-steady-hour",
            what: "an hour at endurance power: does error accumulate, and does anything go \
                   non-finite",
            tracks: vec![track("rider", a, 0.0, vec![S::coast(3600).power(220.0)])],
        },
    ]
}
