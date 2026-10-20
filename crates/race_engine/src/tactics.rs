//! Deterministic, explicit-time utility tactics for the few spotlight riders.
//! This is an input policy: only the existing kernel changes movement and physiology.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Pcg32, RaceSimulationConfig, RaceState, RiderControlInput, RiderId, RiderState};

/// Invalid controller configuration, state or checkpoint.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("tactics: {0}")]
pub struct TacticsError(String);

type Result<T> = std::result::Result<T, TacticsError>;
fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(TacticsError(message.into()))
    }
}

/// Provisional tuning; validated by [`TacticsConfig::new`]. Distances are metres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TacticsSpec {
    /// How often observations may produce a new intention.
    pub decision_interval: Duration,
    /// How long to keep an action unless exhausted or no longer locally valid.
    pub minimum_commitment: Duration,
    /// Required utility improvement to switch an action.
    pub switch_margin: f64,
    /// Enter recovery below this fraction of W′ capacity.
    pub recovery_enter: f64,
    /// Leave recovery only above this fraction.
    pub recovery_exit: f64,
    /// Maximum observed distance ahead/behind.
    pub perception_m: f64,
    /// Preferred following distance, center to center.
    pub wheel_gap_m: f64,
    /// Gap at which following becomes a chase.
    pub chase_gap_m: f64,
    /// Distance before the explicit finish in which sprinting is valid.
    pub sprint_distance_m: f64,
    /// Cruise, recovery, chase and attack requests as fractions of CP.
    pub cruise_cp: f64,
    /// Recovery request as a fraction of CP.
    pub recovery_cp: f64,
    /// Chase ceiling as a fraction of CP.
    pub chase_cp: f64,
    /// Attack request as a fraction of CP.
    pub attack_cp: f64,
    /// Following correction, W per metre of gap error.
    pub gap_gain: f64,
    /// Following correction, W per m/s of speed error.
    pub speed_gain: f64,
}
impl Default for TacticsSpec {
    fn default() -> Self {
        Self {
            decision_interval: Duration::from_secs(1),
            minimum_commitment: Duration::from_secs(3),
            switch_margin: 0.10,
            recovery_enter: 0.20,
            recovery_exit: 0.40,
            perception_m: 100.0,
            wheel_gap_m: 4.0,
            chase_gap_m: 8.0,
            sprint_distance_m: 180.0,
            cruise_cp: 0.90,
            recovery_cp: 0.65,
            chase_cp: 1.25,
            attack_cp: 1.45,
            gap_gain: 8.0,
            speed_gain: 35.0,
        }
    }
}

/// Validated tuning. Not part of the physics configuration and never mutated during a race.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TacticsSpec", into = "TacticsSpec")]
pub struct TacticsConfig(TacticsSpec);
impl TacticsConfig {
    /// Validate finite ranges and timing before any decisions are made.
    pub fn new(s: TacticsSpec) -> Result<Self> {
        require(
            !s.decision_interval.is_zero(),
            "decision interval must be positive",
        )?;
        require(
            s.minimum_commitment >= s.decision_interval,
            "commitment must cover a decision interval",
        )?;
        for x in [s.switch_margin, s.recovery_enter, s.recovery_exit] {
            require(
                x.is_finite() && (0.0..=1.0).contains(&x),
                "fractions must be finite in [0,1]",
            )?;
        }
        require(
            s.recovery_enter < s.recovery_exit,
            "recovery thresholds must be ordered",
        )?;
        for x in [
            s.perception_m,
            s.wheel_gap_m,
            s.chase_gap_m,
            s.sprint_distance_m,
            s.cruise_cp,
            s.recovery_cp,
            s.chase_cp,
            s.attack_cp,
            s.gap_gain,
            s.speed_gain,
        ] {
            require(
                x.is_finite() && x > 0.0,
                "distances, power fractions and gains must be finite and positive",
            )?;
        }
        require(
            s.wheel_gap_m < s.chase_gap_m && s.chase_gap_m < s.perception_m,
            "wheel/chase/perception distances must be ordered",
        )?;
        require(
            s.recovery_cp < 1.0
                && s.recovery_cp < s.cruise_cp
                && s.cruise_cp <= 1.0
                && s.chase_cp > 1.0
                && s.attack_cp >= s.chase_cp
                && s.attack_cp <= 3.0,
            "power fractions must order recovery < cruise <= CP < chase <= attack <= 3 CP",
        )?;
        Ok(Self(s))
    }
    /// Current immutable tuning.
    pub fn spec(&self) -> &TacticsSpec {
        &self.0
    }
}
impl Default for TacticsConfig {
    fn default() -> Self {
        Self::new(TacticsSpec::default()).expect("valid defaults")
    }
}
impl TryFrom<TacticsSpec> for TacticsConfig {
    type Error = TacticsError;
    fn try_from(s: TacticsSpec) -> Result<Self> {
        Self::new(s)
    }
}
impl From<TacticsConfig> for TacticsSpec {
    fn from(c: TacticsConfig) -> Self {
        c.0
    }
}

/// Stable personality, independent of physical capability. Values are in [0,1].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "[f64; 3]", into = "[f64; 3]")]
pub struct TacticalProfile {
    aggression: f64,
    chase_interest: f64,
    conservation: f64,
}
impl TacticalProfile {
    /// Validate a personality without changing CP, W′ or peak power.
    pub fn new(aggression: f64, chase_interest: f64, conservation: f64) -> Result<Self> {
        for x in [aggression, chase_interest, conservation] {
            require(
                x.is_finite() && (0.0..=1.0).contains(&x),
                "personality values must be finite in [0,1]",
            )?;
        }
        Ok(Self {
            aggression,
            chase_interest,
            conservation,
        })
    }
    /// Generate a provisional fixture once, using the repository RNG.
    pub fn seeded(seed: u64) -> Self {
        let mut rng = Pcg32::seeded(seed, 0x74616374);
        Self {
            aggression: rng.next_f64(),
            chase_interest: rng.next_f64(),
            conservation: rng.next_f64(),
        }
    }
}
impl TryFrom<[f64; 3]> for TacticalProfile {
    type Error = TacticsError;
    fn try_from(v: [f64; 3]) -> Result<Self> {
        Self::new(v[0], v[1], v[2])
    }
}
impl From<TacticalProfile> for [f64; 3] {
    fn from(p: TacticalProfile) -> Self {
        [p.aggression, p.chase_interest, p.conservation]
    }
}

/// Bounded planner influence. Local validity and mandatory recovery always win.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TacticalRole {
    /// No influence.
    Neutral,
    /// Up to +0.15 attack utility.
    Instigator,
    /// Up to +0.15 chase utility.
    Chaser,
    /// Up to +0.15 recovery/follow utility.
    Conserving,
}

/// A locally selected intention; declaration order breaks utility ties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TacticalAction {
    /// Replenish reserve.
    Recover,
    /// Follow the closest available wheel.
    Follow,
    /// Close an observable gap.
    Chase,
    /// Sustainable solo/group pace.
    Cruise,
    /// Try to create a gap.
    Attack,
    /// Spend remaining capability at the finish.
    Sprint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intention {
    action: TacticalAction,
    target: Option<String>,
    watts: f64,
    slot: Option<usize>,
    utility: f64,
}

/// A spotlight rider's continuation, separate from kernel state. Save [`Self::checkpoint`]
/// alongside a kernel snapshot and restore both with the same future roles/pack policies.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TacticalController {
    version: u32,
    rider: String,
    config: TacticsConfig,
    profile: TacticalProfile,
    decision_at: Option<Duration>,
    action_since: Duration,
    observed_at: Option<Duration>,
    intention: Option<Intention>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ControllerCheckpoint {
    version: u32,
    rider: String,
    config: TacticsConfig,
    profile: TacticalProfile,
    decision_at: Option<Duration>,
    action_since: Duration,
    observed_at: Option<Duration>,
    intention: Option<Intention>,
}
impl TacticalController {
    /// Start a policy for a configured rider. Physiology is checked on the first observation.
    pub fn new(rider: RiderId, config: TacticsConfig, profile: TacticalProfile) -> Self {
        Self {
            version: 1,
            rider: rider.as_str().into(),
            config,
            profile,
            decision_at: None,
            action_since: Duration::ZERO,
            observed_at: None,
            intention: None,
        }
    }
    /// The last selected action, if this policy has observed the race.
    pub fn action(&self) -> Option<TacticalAction> {
        self.intention.as_ref().map(|i| i.action)
    }
    /// Serialize all policy continuation and fixed tuning, without changing kernel snapshots.
    pub fn checkpoint(&self) -> String {
        serde_json::to_string(self).expect("validated controller")
    }
    /// Restore a checkpoint, rejecting corrupt data rather than silently repairing it.
    pub fn restore(bytes: &str) -> Result<Self> {
        let wire: ControllerCheckpoint =
            serde_json::from_str(bytes).map_err(|e| TacticsError(e.to_string()))?;
        let c = Self {
            version: wire.version,
            rider: wire.rider,
            config: wire.config,
            profile: wire.profile,
            decision_at: wire.decision_at,
            action_since: wire.action_since,
            observed_at: wire.observed_at,
            intention: wire.intention,
        };
        require(
            c.version == 1 && !c.rider.is_empty(),
            "unsupported checkpoint or empty rider",
        )?;
        require(
            c.decision_at.is_some() == c.intention.is_some(),
            "decision and intention must be present together",
        )?;
        if let Some(t) = c.decision_at {
            require(
                c.observed_at.is_some_and(|o| t <= o) && c.action_since <= t,
                "checkpoint times are inconsistent",
            )?;
        } else {
            require(
                c.observed_at.is_none() && c.action_since.is_zero(),
                "unused controller has continuation",
            )?;
        }
        if let Some(i) = &c.intention {
            require(
                i.watts.is_finite() && i.watts >= 0.0 && i.utility.is_finite(),
                "invalid checkpoint intention",
            )?;
            require(
                i.target
                    .as_ref()
                    .is_none_or(|t| !t.is_empty() && t != &c.rider),
                "invalid checkpoint target",
            )?;
            require(
                i.target.is_some()
                    == matches!(i.action, TacticalAction::Follow | TacticalAction::Chase),
                "target does not match action",
            )?;
        }
        Ok(c)
    }

    /// Observe one immutable pre-step state and produce an ordinary kernel input. Repeated calls
    /// at the same time are stable; backwards time is rejected. Exhaustion interrupts commitment.
    pub fn input(
        &mut self,
        config: &RaceSimulationConfig,
        state: &RaceState,
        role: TacticalRole,
    ) -> Result<RiderControlInput> {
        config
            .validate_state(state, "tactics.state")
            .map_err(|e| TacticsError(e.to_string()))?;
        let now = state.simulation_time();
        require(
            self.observed_at.is_none_or(|t| now >= t),
            "simulation time moved backwards",
        )?;
        let id = RiderId::new(&self.rider).map_err(|e| TacticsError(e.to_string()))?;
        let me = state
            .rider_state(&id)
            .ok_or_else(|| TacticsError("rider absent from state".into()))?;
        let definition = config.definition_for(&id).expect("validated state");
        let physiology = definition
            .physiology()
            .ok_or_else(|| TacticsError("spotlight rider needs physiology".into()))?;
        let reserve =
            me.w_prime_balance_j().expect("validated physiology") / physiology.w_prime_j();
        let s = self.config.spec();
        if let Some(drafting) = config.drafting() {
            require(
                s.wheel_gap_m > drafting.min_gap_m() && s.wheel_gap_m < drafting.max_gap_m(),
                "preferred wheel gap must fit the configured drafting range",
            )?;
        }
        if let Some(i) = &self.intention {
            require(
                i.slot.is_none_or(|v| v < config.lateral_slot_count()),
                "checkpoint slot outside configuration",
            )?;
            require(
                i.target
                    .as_ref()
                    .is_none_or(|t| state.riders().iter().any(|r| r.id().as_str() == t)),
                "checkpoint target absent from configuration",
            )?;
        }
        let due = self
            .decision_at
            .is_none_or(|t| now - t >= s.decision_interval);
        let emergency =
            reserve < s.recovery_enter && self.action() != Some(TacticalAction::Recover);
        if due || emergency {
            let mut candidates = self.candidates(config, state, me, reserve, role);
            let recovering = reserve < s.recovery_enter
                || (self.action() == Some(TacticalAction::Recover) && reserve < s.recovery_exit);
            if recovering {
                candidates.retain(|i| i.action == TacticalAction::Recover);
            }
            let mut chosen = candidates[0].clone();
            for candidate in &candidates[1..] {
                if candidate.utility > chosen.utility {
                    chosen = candidate.clone();
                }
            }
            if !recovering {
                if let Some(previous) = &self.intention {
                    if let Some(current) = candidates
                        .iter()
                        .find(|c| c.action == previous.action && c.target == previous.target)
                    {
                        let committed = now - self.action_since < s.minimum_commitment;
                        if committed || chosen.utility <= current.utility + s.switch_margin {
                            chosen = current.clone();
                        }
                    }
                }
            }
            if self
                .intention
                .as_ref()
                .is_none_or(|i| i.action != chosen.action || i.target != chosen.target)
            {
                self.action_since = now;
            }
            self.intention = Some(chosen);
            self.decision_at = Some(now);
        }
        let i = self.intention.as_ref().expect("first observation decides");
        let target = i
            .target
            .as_ref()
            .map(RiderId::new)
            .transpose()
            .map_err(|e| TacticsError(e.to_string()))?;
        let output = RiderControlInput::new(id, i.watts, 0.0, i.slot.map(|v| v as i64), target)
            .map_err(|e| TacticsError(e.to_string()))?;
        self.observed_at = Some(now);
        Ok(output)
    }

    fn candidates(
        &self,
        config: &RaceSimulationConfig,
        state: &RaceState,
        me: &RiderState,
        reserve: f64,
        role: TacticalRole,
    ) -> Vec<Intention> {
        let s = self.config.spec();
        let p = config
            .definition_for(me.id())
            .unwrap()
            .physiology()
            .unwrap();
        let cp = p.critical_power_w();
        let ceiling = cp + (p.peak_power_w() - cp) * reserve;
        let mut result = Vec::new();
        let mut add = |action, target: Option<&RiderState>, watts: f64, utility: f64| {
            let bias = match (role, action) {
                (TacticalRole::Instigator, TacticalAction::Attack)
                | (TacticalRole::Chaser, TacticalAction::Chase)
                | (TacticalRole::Conserving, TacticalAction::Recover | TacticalAction::Follow) => {
                    0.15
                }
                _ => 0.0,
            };
            result.push(Intention {
                action,
                target: target.map(|r| r.id().as_str().into()),
                watts: watts.clamp(0.0, ceiling),
                slot: target.map(RiderState::lateral_slot),
                utility: utility + bias,
            });
        };
        add(
            TacticalAction::Recover,
            None,
            s.recovery_cp * cp,
            0.10 + self.profile.conservation * (1.0 - reserve) * 0.5,
        );
        let unfinished = |r: &&RiderState| {
            config
                .terrain()
                .explicit_length_m()
                .is_none_or(|end| r.distance_m() < end)
        };
        let ahead = state
            .riders()
            .iter()
            .filter(unfinished)
            .filter(|r| {
                r.distance_m() > me.distance_m()
                    && r.distance_m() - me.distance_m() <= s.perception_m
            })
            .min_by(|a, b| {
                a.distance_m()
                    .total_cmp(&b.distance_m())
                    .then_with(|| a.id().cmp(b.id()))
            });
        if let Some(target) = ahead {
            let gap = target.distance_m() - me.distance_m();
            let watts = cp * s.cruise_cp
                + s.gap_gain * (gap - s.wheel_gap_m)
                + s.speed_gain * (target.speed_ms() - me.speed_ms());
            let action = if gap <= s.chase_gap_m {
                TacticalAction::Follow
            } else {
                TacticalAction::Chase
            };
            if action == TacticalAction::Follow {
                add(
                    action,
                    Some(target),
                    watts.min(s.chase_cp * cp),
                    0.65 + 0.15 * self.profile.conservation,
                );
            } else if reserve >= s.recovery_enter {
                // A visibly opening gap is more threatening, without identifying a human or workout.
                let opening = ((target.speed_ms() - me.speed_ms()) / 3.0).clamp(0.0, 1.0);
                add(
                    action,
                    Some(target),
                    watts.min(s.chase_cp * cp),
                    0.50 + 0.25 * self.profile.chase_interest + 0.10 * opening,
                );
            }
        }
        add(TacticalAction::Cruise, None, s.cruise_cp * cp, 0.40);
        let nearby = state.riders().iter().filter(unfinished).any(|r| {
            r.id() != me.id() && (r.distance_m() - me.distance_m()).abs() <= s.perception_m
        });
        if nearby && reserve >= s.recovery_exit {
            add(
                TacticalAction::Attack,
                None,
                s.attack_cp * cp,
                0.25 + 0.45 * self.profile.aggression * reserve,
            );
        }
        if config.terrain().explicit_length_m().is_some_and(|end| {
            me.distance_m() < end && end - me.distance_m() <= s.sprint_distance_m
        }) && reserve >= s.recovery_enter
        {
            add(TacticalAction::Sprint, None, ceiling, 1.10);
        }
        result
    }
}
