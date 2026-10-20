//! The race view's live kernel: the same deterministic `RaceSimulation` the headless tests step,
//! stepped in real time at its fixed step and interpolated for display.
//!
//! The riders' powers are a scripted fixture (the prototype's replay scenario), not AI and not
//! human telemetry: tactical AI and feeding a real trainer into the kernel are later slices.
//! Positions and speeds come only from the kernel.

use std::time::Duration;

use race_engine::{
    BeyondEndBehavior, Pcg32, RaceInputFrame, RaceSimulation, RaceSimulationConfig,
    RaceSimulationConfigSpec, RaceState, RiderControlInput, RiderDefinition, RiderId, RiderState,
    SimulationTerrain, TerrainInterpolation, TerrainSample,
};
use race_physics::{LongitudinalDynamicsConfig, RiderPhysicalParams, SimulationTimestep};

use crate::app::route::elevation_table;

/// `(from_step, watts)`: the latest entry at or before a step applies.
type PowerScript = &'static [(u64, f64)];

const SCRIPT: [(&str, f64, f64, i64, f64, PowerScript); 3] = [
    // id, mass, start distance, slot, start speed, power script
    ("human", 78.0, 30.0, 1, 8.0, &[(0, 250.0), (650, 330.0)]),
    ("rival", 77.0, 35.0, 2, 8.0, &[(0, 275.0), (900, 245.0)]),
    ("leader", 75.0, 42.0, 0, 8.0, &[(0, 265.0)]),
];

/// The id the camera follows.
pub const PLAYER_ID: &str = "human";

/// One rider, as displayed.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceRider {
    /// Id.
    pub id: String,
    /// Distance, m, interpolated between kernel steps.
    pub distance_m: f64,
    /// Speed, m/s, interpolated likewise.
    pub speed_ms: f64,
    /// Lateral slot.
    pub slot: usize,
    /// Whose wheel it is sheltering behind.
    pub draft_target: Option<String>,
}

/// The live race.
#[derive(Debug)]
pub struct LiveRace {
    simulation: RaceSimulation,
    previous: RaceState,
    current: RaceState,
    step: u64,
    /// Real time not yet stepped, s.
    accumulator: f64,
    finished: bool,
}

fn terrain() -> SimulationTerrain {
    let samples = [
        (0.0, 0.005),
        (180.0, 0.025),
        (430.0, -0.01),
        (760.0, 0.03),
        (1120.0, -0.015),
    ]
    .map(|(d, g)| TerrainSample::new(d, g).expect("valid sample"))
    .to_vec();
    SimulationTerrain::new(
        samples,
        TerrainInterpolation::Linear,
        BeyondEndBehavior::HoldLastGrade,
        Some(2000.0),
    )
    .expect("valid terrain")
}

impl Default for LiveRace {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveRace {
    /// The scenario at its start.
    pub fn new() -> Self {
        let config = RaceSimulationConfig::new(RaceSimulationConfigSpec {
            fixed_step: SimulationTimestep::RECOMMENDED,
            riders: SCRIPT
                .iter()
                .map(|(id, mass, ..)| {
                    RiderDefinition::new(
                        RiderId::new(*id).unwrap(),
                        RiderPhysicalParams::new(*mass),
                    )
                    .unwrap()
                })
                .collect(),
            terrain: terrain(),
            dynamics: LongitudinalDynamicsConfig::default(),
            drafting: None,
            lateral_slot_count: 3,
            air_density_kg_m3: 1.225,
            headwind_ms: 0.0,
        })
        .expect("valid scenario");
        let state = RaceState::new(
            Duration::ZERO,
            Pcg32::seeded(20_260_926, 1).state(),
            SCRIPT
                .iter()
                .map(|(id, _, d, slot, v, _)| {
                    RiderState::new(RiderId::new(*id).unwrap(), *d, *v, *slot, None).unwrap()
                })
                .collect(),
        )
        .expect("valid start");
        Self {
            simulation: RaceSimulation::new(config).expect("steppable"),
            previous: state.clone(),
            current: state,
            step: 0,
            accumulator: 0.0,
            finished: false,
        }
    }

    fn step_seconds(&self) -> f64 {
        self.simulation.config().fixed_step().as_secs_f64()
    }

    fn inputs(&self) -> RaceInputFrame {
        RaceInputFrame::new(
            SCRIPT
                .iter()
                .map(|(id, _, _, _, _, script)| {
                    let watts = script
                        .iter()
                        .rev()
                        .find(|(from, _)| *from <= self.step)
                        .map_or(0.0, |(_, w)| *w);
                    RiderControlInput::power(RiderId::new(*id).unwrap(), watts).unwrap()
                })
                .collect(),
        )
        .expect("one input per rider")
    }

    /// Adds `real_seconds` of wall time and steps every whole fixed step it completes. A long
    /// stall is capped at a quarter second rather than replayed in one frame.
    pub fn advance(&mut self, real_seconds: f64) {
        if self.finished {
            return;
        }
        let dt = self.simulation.config().fixed_step();
        self.accumulator += real_seconds.clamp(0.0, 0.25);
        while self.accumulator >= self.step_seconds() {
            self.accumulator -= self.step_seconds();
            let next = self
                .simulation
                .step(&self.current, &self.inputs(), dt)
                .expect("the scripted scenario is valid")
                .next_state;
            self.previous = std::mem::replace(&mut self.current, next);
            self.step += 1;
            if self
                .current
                .riders()
                .iter()
                .any(|r| r.distance_m() >= self.length_m())
            {
                self.finished = true;
                self.accumulator = 0.0;
                break;
            }
        }
    }

    /// Kernel time, s, interpolated.
    pub fn time_s(&self) -> f64 {
        self.previous.simulation_time().as_secs_f64() + self.alpha() * self.step_seconds()
    }

    fn alpha(&self) -> f64 {
        if self.finished || self.step == 0 {
            if self.finished {
                1.0
            } else {
                0.0
            }
        } else {
            (self.accumulator / self.step_seconds()).clamp(0.0, 1.0)
        }
    }

    /// The riders between the last two kernel states.
    pub fn riders(&self) -> Vec<RaceRider> {
        let a = self.alpha();
        self.current
            .riders()
            .iter()
            .zip(self.previous.riders())
            .map(|(now, before)| RaceRider {
                id: now.id().as_str().to_owned(),
                distance_m: before.distance_m() + (now.distance_m() - before.distance_m()) * a,
                speed_ms: before.speed_ms() + (now.speed_ms() - before.speed_ms()) * a,
                slot: now.lateral_slot(),
                draft_target: now.draft_target_id().map(|t| t.as_str().to_owned()),
            })
            .collect()
    }

    /// Whether a rider has reached the end.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Course length, m.
    pub fn length_m(&self) -> f64 {
        self.simulation.config().terrain().end_distance_m()
    }

    /// Lateral slots.
    pub fn slot_count(&self) -> usize {
        self.simulation.config().lateral_slot_count()
    }

    /// Elevation every 5 m.
    pub fn elevations(&self) -> Vec<f64> {
        elevation_table(self.simulation.config().terrain(), self.length_m() + 300.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_time_slices_do_not_change_the_race() {
        let mut coarse = LiveRace::new();
        let mut fine = LiveRace::new();
        for _ in 0..200 {
            coarse.advance(0.25);
        }
        for _ in 0..3000 {
            fine.advance(1.0 / 60.0);
        }
        // 50 s of kernel time either way: identical kernel states, bit for bit.
        assert_eq!(coarse.step, 1000);
        assert_eq!(fine.step, 1000);
        assert_eq!(coarse.current, fine.current);
    }

    #[test]
    fn the_scripted_attack_changes_the_order_and_the_race_ends() {
        let mut race = LiveRace::new();
        while !race.is_finished() {
            race.advance(0.25);
            assert!(race.step < 20_000, "ends");
        }
        let riders = race.riders();
        let human = riders.iter().find(|r| r.id == PLAYER_ID).unwrap();
        assert!(riders.iter().any(|r| r.distance_m >= race.length_m()));
        assert!(human.speed_ms > 5.0);
    }
}
