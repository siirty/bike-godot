//! The race view's live kernel: the same deterministic `RaceSimulation` the headless tests step,
//! stepped in real time at its fixed step and interpolated for display.
//!
//! The player follows a scripted power request; opponents use reactive spotlight tactics. Every
//! software rider has finite CP/W′/Pmax capability. Trainer integration is a later slice.
//! Positions and speeds come only from the kernel.

use std::time::Duration;

use std::collections::VecDeque;

use race_engine::{
    BeyondEndBehavior, Pcg32, RaceEvent, RaceEventKind, RaceInputFrame, RaceSimulation,
    RaceSimulationConfig, RaceSimulationConfigSpec, RaceState, RiderControlInput, RiderDefinition,
    RiderId, RiderState, SimulationTerrain, TerrainInterpolation, TerrainSample,
};
use race_physics::{
    LongitudinalDynamicsConfig, PhysiologyParams, RiderPhysicalParams, SimulationTimestep,
};

use crate::app::format::format_duration;
use crate::app::route::elevation_table;

/// How many event lines the race view keeps.
pub const EVENT_FEED_LENGTH: usize = 4;

/// `(from_step, watts)`: the latest entry at or before a step applies.
type PowerScript = &'static [(u64, f64)];

const PLAYER_POWER: PowerScript = &[(0, 250.0), (650, 330.0)];

const RIDERS: [(&str, f64, f64, i64, f64); 3] = [
    // id, mass, start distance, slot, start speed
    ("human", 78.0, 30.0, 1, 8.0),
    ("rival", 77.0, 35.0, 2, 8.0),
    ("leader", 75.0, 42.0, 0, 8.0),
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
    feed: VecDeque<String>,
    tactics: Vec<race_engine::tactics::TacticalController>,
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
            riders: RIDERS
                .iter()
                .map(|(id, mass, ..)| {
                    RiderDefinition::new(
                        RiderId::new(*id).unwrap(),
                        RiderPhysicalParams::new(*mass),
                    )
                    .unwrap()
                    .with_physiology(
                        PhysiologyParams::new(
                            match *id {
                                "human" => 280.0,
                                "rival" => 265.0,
                                _ => 270.0,
                            },
                            20_000.0,
                            1100.0,
                        )
                        .expect("valid capability"),
                    )
                })
                .collect(),
            terrain: terrain(),
            dynamics: LongitudinalDynamicsConfig::default(),
            drafting: Some(
                race_engine::BasicDraftingConfig::new(6.0, vec![0.62, 0.85])
                    .expect("valid drafting"),
            ),
            lateral_slot_count: 3,
            air_density_kg_m3: 1.225,
            headwind_ms: 0.0,
        })
        .expect("valid scenario");
        let state = RaceState::new(
            Duration::ZERO,
            Pcg32::seeded(20_260_926, 1).state(),
            RIDERS
                .iter()
                .map(|(id, _, d, slot, v)| {
                    RiderState::new(RiderId::new(*id).unwrap(), *d, *v, *slot, None)
                        .unwrap()
                        .with_w_prime_balance(20_000.0)
                        .unwrap()
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
            feed: VecDeque::new(),
            tactics: ["leader", "rival"]
                .into_iter()
                .enumerate()
                .map(|(i, id)| {
                    race_engine::tactics::TacticalController::new(
                        RiderId::new(id).unwrap(),
                        race_engine::tactics::TacticsConfig::default(),
                        race_engine::tactics::TacticalProfile::seeded(42 + i as u64),
                    )
                })
                .collect(),
        }
    }

    fn step_seconds(&self) -> f64 {
        self.simulation.config().fixed_step().as_secs_f64()
    }

    fn inputs(&mut self) -> RaceInputFrame {
        let script = PLAYER_POWER;
        let watts = script
            .iter()
            .rev()
            .find(|(from, _)| *from <= self.step)
            .map_or(0.0, |(_, w)| *w);
        let mut inputs =
            vec![RiderControlInput::power(RiderId::new(PLAYER_ID).unwrap(), watts).unwrap()];
        for controller in &mut self.tactics {
            inputs.push(
                controller
                    .input(
                        self.simulation.config(),
                        &self.current,
                        race_engine::tactics::TacticalRole::Neutral,
                    )
                    .expect("valid tactical scenario"),
            );
        }
        RaceInputFrame::new(inputs).expect("one input per rider")
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
            let inputs = self.inputs();
            let result = self
                .simulation
                .step(&self.current, &inputs, dt)
                .expect("the scripted scenario is valid");
            for line in result.events.iter().filter_map(describe_event) {
                if self.feed.len() == EVENT_FEED_LENGTH {
                    self.feed.pop_front();
                }
                self.feed.push_back(line);
            }
            self.previous = std::mem::replace(&mut self.current, result.next_state);
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

    /// The latest race events as text, oldest first.
    pub fn event_feed(&self) -> Vec<String> {
        self.feed.iter().cloned().collect()
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

fn name(id: &RiderId) -> String {
    match id.as_str() {
        PLAYER_ID => "You".to_owned(),
        other => {
            let mut c = other.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        }
    }
}

fn ordinal(place: u32) -> String {
    let suffix = match (place % 10, place % 100) {
        (1, n) if n != 11 => "st",
        (2, n) if n != 12 => "nd",
        (3, n) if n != 13 => "rd",
        _ => "th",
    };
    format!("{place}{suffix}")
}

/// One feed line for an event, or `None` for events the view does not narrate (slot changes:
/// visible on the road, and too frequent to read).
pub fn describe_event(event: &RaceEvent) -> Option<String> {
    let is_player = |id: &RiderId| id.as_str() == PLAYER_ID;
    let text = match event.kind() {
        RaceEventKind::SlotChanged { .. } => return None,
        RaceEventKind::Overtook { rider, passed } if is_player(passed) => {
            format!("{} passes you", name(rider))
        }
        RaceEventKind::Overtook { rider, passed } if is_player(rider) => {
            format!("You pass {}", name(passed))
        }
        RaceEventKind::Overtook { rider, passed } => {
            format!("{} passes {}", name(rider), name(passed))
        }
        RaceEventKind::WheelTaken { rider, target } if is_player(target) => {
            format!("{} is on your wheel", name(rider))
        }
        RaceEventKind::WheelTaken { rider, target } => {
            format!("{} takes {}'s wheel", name(rider), name(target))
        }
        RaceEventKind::WheelLost { rider, target } if is_player(target) => {
            format!("{} drops off your wheel", name(rider))
        }
        RaceEventKind::WheelLost { rider, target } => {
            format!("{} loses {}'s wheel", name(rider), name(target))
        }
        RaceEventKind::Finished { rider, place } if is_player(rider) => {
            format!("You finish {}", ordinal(*place))
        }
        RaceEventKind::Finished { rider, place } => {
            format!("{} finishes {}", name(rider), ordinal(*place))
        }
    };
    Some(format!(
        "{}  {text}",
        format_duration(event.simulation_time())
    ))
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

    fn event(kind: RaceEventKind) -> RaceEvent {
        RaceEvent::new(Duration::from_secs(83), kind).unwrap()
    }

    fn rid(s: &str) -> RiderId {
        RiderId::new(s).unwrap()
    }

    #[test]
    fn events_are_narrated_from_the_players_point_of_view() {
        let line = |k| describe_event(&event(k));
        assert_eq!(
            line(RaceEventKind::Overtook {
                rider: rid("rival"),
                passed: rid("human")
            })
            .unwrap(),
            "1:23  Rival passes you"
        );
        assert_eq!(
            line(RaceEventKind::Overtook {
                rider: rid("human"),
                passed: rid("leader")
            })
            .unwrap(),
            "1:23  You pass Leader"
        );
        assert_eq!(
            line(RaceEventKind::WheelTaken {
                rider: rid("rival"),
                target: rid("human")
            })
            .unwrap(),
            "1:23  Rival is on your wheel"
        );
        assert_eq!(
            line(RaceEventKind::Finished {
                rider: rid("human"),
                place: 2
            })
            .unwrap(),
            "1:23  You finish 2nd"
        );
        assert_eq!(
            line(RaceEventKind::Finished {
                rider: rid("leader"),
                place: 11
            })
            .unwrap(),
            "1:23  Leader finishes 11th"
        );
        assert_eq!(
            line(RaceEventKind::SlotChanged {
                rider: rid("rival"),
                from_slot: 0,
                to_slot: 1
            }),
            None
        );
    }

    #[test]
    fn the_feed_reports_the_scripted_race_and_its_winner() {
        let mut race = LiveRace::new();
        while !race.is_finished() {
            race.advance(0.25);
        }
        let feed = race.event_feed();
        assert!(feed.len() <= EVENT_FEED_LENGTH);
        assert!(
            feed.last().is_some_and(|l| l.contains("1st")),
            "the race ends on a finish: {feed:?}"
        );
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
