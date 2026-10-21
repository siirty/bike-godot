//! Reference motor execution, separate from tactical interpretation and workout authoring.
use crate::{environment, Command, Error};
use race_engine::{RaceInputFrame, RaceSimulationConfig, RaceState, RiderControlInput};
use race_physics::power_for_speed;

/// A motion objective contains only observable geometry, never a workout intensity.
#[derive(Debug, Clone, Copy)]
pub struct MotionGoal {
    /// Desired ground speed in m/s.
    pub speed_ms: f64,
    /// Longitudinal position error to correct, m; positive means close forward.
    pub error_m: f64,
    /// Requested lateral slot.
    pub slot: i64,
}
/// Track observable motion using ordinary crank/brake intentions and fixed own capability.
/// 0.12/s gap gain, 1 m/s closing bound and 2 s speed response are synthetic test settings.
pub fn track(
    config: &RaceSimulationConfig,
    state: &RaceState,
    rider: &str,
    goal: MotionGoal,
) -> Result<Command, Error> {
    config.validate_state(state, "motor")?;
    if !goal.speed_ms.is_finite()
        || goal.speed_ms < 0.0
        || !goal.error_m.is_finite()
        || goal.slot < 0
        || goal.slot >= config.lateral_slot_count() as i64
    {
        return Err("invalid motion goal".into());
    }
    let i = state
        .riders()
        .iter()
        .position(|r| r.id().as_str() == rider)
        .ok_or("unknown motor rider")?;
    let h = &state.riders()[i];
    let own = &config.riders()[i];
    let cap = own.physiology().ok_or("motor requires own physiology")?;
    let controls = RaceInputFrame::new(
        state
            .riders()
            .iter()
            .enumerate()
            .map(|(j, r)| {
                RiderControlInput::new(
                    r.id().clone(),
                    0.0,
                    0.0,
                    Some(if j == i {
                        goal.slot
                    } else {
                        r.lateral_slot() as i64
                    }),
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let slots = race_engine::simulation::positioning::resolve_lateral_slots(state, &controls);
    let targets = race_engine::simulation::positioning::select_draft_targets(
        config, state, &slots, &controls,
    );
    let drags = race_engine::simulation::positioning::compute_drag_multipliers(
        config, state, &slots, &targets,
    );
    let mut physical = *own.physical();
    physical.cda_m2 *= drags[i];
    let steady = power_for_speed(
        h.speed_ms(),
        &physical,
        &environment(config, h.distance_m()),
    );
    let desired_speed = (goal.speed_ms + (0.12 * goal.error_m).clamp(-1.0, 1.0)).max(0.0);
    let force = physical.total_mass_kg * (desired_speed - h.speed_ms()) / 2.0;
    // A small control-speed floor permits a positive launch request from rest;
    // the production kernel still resolves the actual low-speed propulsive force.
    let watts = steady + force * h.speed_ms().max(0.5) / physical.drive_train_efficiency;
    // At low speed watts cannot represent a braking force; braking remains an explicit force.
    let brake = if h.speed_ms() > 0.1 && watts < 0.0 {
        -watts * physical.drive_train_efficiency / h.speed_ms()
    } else {
        0.0
    };
    Ok(Command {
        power: watts.clamp(0.0, cap.peak_power_w()),
        brake,
        slot: goal.slot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use race_engine::*;
    use race_physics::{LongitudinalDynamicsConfig, PhysiologyParams, RiderPhysicalParams};
    use std::time::Duration;
    fn world(gap: f64, grade: f64) -> (RaceSimulationConfig, RaceState) {
        let riders = ["a", "b"]
            .into_iter()
            .map(|id| {
                RiderDefinition::new(RiderId::new(id).unwrap(), RiderPhysicalParams::new(85.0))
                    .unwrap()
                    .with_physiology(PhysiologyParams::new(250.0, 20000.0, 1100.0).unwrap())
            })
            .collect();
        let config = RaceSimulationConfig::new(RaceSimulationConfigSpec {
            fixed_step: Duration::from_millis(50),
            riders,
            terrain: SimulationTerrain::new(
                vec![TerrainSample::new(0.0, grade).unwrap()],
                TerrainInterpolation::Linear,
                BeyondEndBehavior::HoldLastGrade,
                None,
            )
            .unwrap(),
            dynamics: LongitudinalDynamicsConfig::default(),
            drafting: None,
            lateral_slot_count: 1,
            air_density_kg_m3: 1.225,
            headwind_ms: 0.0,
        })
        .unwrap();
        let state = RaceState::new(
            Duration::ZERO,
            Pcg32::seeded(1, 1).state(),
            [0.0, gap]
                .into_iter()
                .enumerate()
                .map(|(i, d)| {
                    RiderState::new(config.riders()[i].id().clone(), d, 8.0, 0, None)
                        .unwrap()
                        .with_w_prime_balance(20000.0)
                        .unwrap()
                })
                .collect(),
        )
        .unwrap();
        (config, state)
    }
    #[test]
    fn equilibrium_matches_independent_power_balance() {
        let (c, s) = world(30.0, 0.0);
        let cmd = track(
            &c,
            &s,
            "a",
            MotionGoal {
                speed_ms: 8.0,
                error_m: 0.0,
                slot: 0,
            },
        )
        .unwrap();
        let expected = power_for_speed(8.0, c.riders()[0].physical(), &crate::environment(&c, 0.0));
        assert!((cmd.power - expected).abs() < 1e-9);
        assert_eq!(cmd.brake, 0.0);
    }
    #[test]
    fn a_gap_closes_without_a_workout_trace_or_teleporting() {
        let (c, mut s) = world(24.0, 0.0);
        let sim = RaceSimulation::new(c.clone()).unwrap();
        for _ in 0..2400 {
            let a = &s.riders()[0];
            let b = &s.riders()[1];
            let ca = track(
                &c,
                &s,
                "a",
                MotionGoal {
                    speed_ms: b.speed_ms(),
                    error_m: b.distance_m() - a.distance_m() - 4.0,
                    slot: 0,
                },
            )
            .unwrap();
            let cb = track(
                &c,
                &s,
                "b",
                MotionGoal {
                    speed_ms: 8.0,
                    error_m: 0.0,
                    slot: 0,
                },
            )
            .unwrap();
            assert!(ca.power <= 1100.0 && ca.power >= 0.0);
            let frame = RaceInputFrame::new(vec![
                RiderControlInput::new(
                    c.riders()[0].id().clone(),
                    ca.power,
                    ca.brake,
                    Some(0),
                    None,
                )
                .unwrap(),
                RiderControlInput::new(
                    c.riders()[1].id().clone(),
                    cb.power,
                    cb.brake,
                    Some(0),
                    None,
                )
                .unwrap(),
            ])
            .unwrap();
            let next = sim.step(&s, &frame, c.fixed_step()).unwrap().next_state;
            assert!(next.riders()[0].distance_m() - a.distance_m() < 0.6);
            s = next;
        }
        assert!((s.riders()[1].distance_m() - s.riders()[0].distance_m() - 4.0).abs() < 0.1);
        assert!((s.riders()[0].speed_ms() - 8.0).abs() < 0.01);
    }
    #[test]
    fn positive_motion_goal_can_launch_from_rest() {
        let (c, s) = world(20.0, 0.0);
        let stopped = RaceState::new(
            s.simulation_time(),
            s.random_state(),
            s.riders()
                .iter()
                .map(|r| {
                    RiderState::new(r.id().clone(), r.distance_m(), 0.0, 0, None)
                        .unwrap()
                        .with_w_prime_balance(20000.0)
                        .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let cmd = track(
            &c,
            &stopped,
            "a",
            MotionGoal {
                speed_ms: 5.0,
                error_m: 0.0,
                slot: 0,
            },
        )
        .unwrap();
        assert!(cmd.power > 0.0);
        let sim = RaceSimulation::new(c.clone()).unwrap();
        let frame = RaceInputFrame::new(vec![
            RiderControlInput::power(c.riders()[0].id().clone(), cmd.power).unwrap(),
            RiderControlInput::power(c.riders()[1].id().clone(), 0.0).unwrap(),
        ])
        .unwrap();
        assert!(
            sim.step(&stopped, &frame, c.fixed_step())
                .unwrap()
                .next_state
                .riders()[0]
                .speed_ms()
                > 0.0
        );
    }
    #[test]
    fn downhill_slowing_uses_brakes_and_invalid_goals_are_rejected() {
        let (c, s) = world(20.0, -0.05);
        let cmd = track(
            &c,
            &s,
            "a",
            MotionGoal {
                speed_ms: 4.0,
                error_m: 0.0,
                slot: 0,
            },
        )
        .unwrap();
        assert_eq!(cmd.power, 0.0);
        assert!(cmd.brake > 0.0);
        assert!(track(
            &c,
            &s,
            "a",
            MotionGoal {
                speed_ms: f64::NAN,
                error_m: 0.0,
                slot: 0
            }
        )
        .is_err());
        assert!(track(
            &c,
            &s,
            "missing",
            MotionGoal {
                speed_ms: 8.0,
                error_m: 0.0,
                slot: 0
            }
        )
        .is_err());
    }
}
