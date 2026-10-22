//! Semantic projection of actual geometry, outside kernel events and snapshots.
use crate::{RaceSimulationConfig, RaceState};
use serde::Serialize;
use std::{collections::VecDeque, time::Duration};

/// Debounced longitudinal topology; thresholds are provisional engineering settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Topology {
    /// All riders form one connected group.
    Together,
    /// Membership changed less than three seconds ago.
    Forming,
    /// Two persistent groups.
    Established,
    /// More than two persistent groups.
    MultipleGroups,
    /// A physical reduction in group count, displayed for three seconds.
    Regrouping,
}
/// Explicit policy commitment concerning the front and immediately following group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ChaseIntent {
    /// No declared organised pursuit.
    None,
    /// Deliberate gap management, never inferred from a stable gap.
    Manage,
    /// Attempt to close the gap.
    Pursue,
}
/// Gap classification is distinct from topology and an authored workout annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Pursuit {
    /// No group separation exists.
    NoSeparation,
    /// No explicit pursuit policy, regardless of numeric trend.
    Unchallenged,
    /// Explicit gap regulation.
    Managed,
    /// Insufficient stable-membership history.
    Observing,
    /// Gap closing faster than the deadband.
    Closing,
    /// Gap within the deadband under pursuit.
    Stable,
    /// Gap growing under pursuit.
    LosingGround,
}
/// Derived group membership and physical extent, frontmost group first.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    members: Vec<String>,
    front_m: f64,
    rear_m: f64,
}
impl Group {
    /// Canonical member identities.
    pub fn members(&self) -> &[String] {
        &self.members
    }
}
/// Read-only inspector sample; never activity telemetry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Observation {
    topology: Topology,
    pursuit: Pursuit,
    groups: Vec<Group>,
    gap_m: Option<f64>,
    gap_rate_ms: Option<f64>,
}
impl Observation {
    /// Debounced topology.
    pub fn topology(&self) -> Topology {
        self.topology
    }
    /// Explicit intent plus independently measured gap trend.
    pub fn pursuit(&self) -> Pursuit {
        self.pursuit
    }
    /// Current geometry, independent of debounce.
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }
    /// Rear of the front group to front of the next group.
    pub fn gap_m(&self) -> Option<f64> {
        self.gap_m
    }
    /// Negative means closing; membership changes clear history.
    pub fn gap_rate_ms(&self) -> Option<f64> {
        self.gap_rate_ms
    }
}
/// Validated, configurable classifier thresholds.
#[derive(Debug, Clone, Copy)]
pub struct ObserverTuning {
    gap_m: f64,
    persistence: Duration,
    window: Duration,
    deadband: f64,
}
impl ObserverTuning {
    /// Require finite positive distance/times and finite nonnegative deadband.
    pub fn new(
        gap_m: f64,
        persistence: Duration,
        window: Duration,
        deadband: f64,
    ) -> Result<Self, &'static str> {
        if !gap_m.is_finite()
            || gap_m <= 0.0
            || persistence.is_zero()
            || window.is_zero()
            || !deadband.is_finite()
            || deadband < 0.0
        {
            return Err("invalid observer thresholds");
        }
        Ok(Self {
            gap_m,
            persistence,
            window,
            deadband,
        })
    }
}
impl Default for ObserverTuning {
    fn default() -> Self {
        Self::new(12.0, Duration::from_secs(3), Duration::from_secs(5), 0.15).unwrap()
    }
}
/// Bounded continuation for one immutable race configuration.
#[derive(Debug, Clone, Default)]
pub struct RaceObserver {
    tuning: ObserverTuning,
    membership: Vec<Vec<String>>,
    changed: Duration,
    merge: bool,
    history: VecDeque<(Duration, f64)>,
    last: Option<(RaceState, ChaseIntent, Observation)>,
}
impl RaceObserver {
    /// Start with explicit validated thresholds.
    pub fn new(tuning: ObserverTuning) -> Self {
        Self {
            tuning,
            ..Self::default()
        }
    }
    /// Observe monotonically increasing simulation times; conflicting duplicate samples fail atomically.
    pub fn observe(
        &mut self,
        config: &RaceSimulationConfig,
        state: &RaceState,
        intent: ChaseIntent,
    ) -> Result<Observation, &'static str> {
        config
            .validate_state(state, "observer")
            .map_err(|_| "invalid observation state")?;
        let now = state.simulation_time();
        if let Some((previous, old_intent, view)) = &self.last {
            if now < previous.simulation_time() {
                return Err("observation time moved backwards");
            }
            if now == previous.simulation_time() {
                return if previous == state && *old_intent == intent {
                    Ok(view.clone())
                } else {
                    Err("conflicting observation at the same time")
                };
            }
        }
        let mut riders: Vec<_> = state.riders().iter().collect();
        riders.sort_by(|a, b| {
            b.distance_m()
                .total_cmp(&a.distance_m())
                .then_with(|| a.id().cmp(b.id()))
        });
        let mut groups: Vec<Group> = vec![];
        for rider in riders {
            if let Some(group) = groups
                .last_mut()
                .filter(|g| g.rear_m - rider.distance_m() <= self.tuning.gap_m)
            {
                group.members.push(rider.id().as_str().into());
                group.rear_m = rider.distance_m();
            } else {
                groups.push(Group {
                    members: vec![rider.id().as_str().into()],
                    front_m: rider.distance_m(),
                    rear_m: rider.distance_m(),
                });
            }
        }
        for g in &mut groups {
            g.members
                .sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        }
        let members: Vec<_> = groups.iter().map(|g| g.members.clone()).collect();
        if members != self.membership {
            self.merge = !self.membership.is_empty() && members.len() < self.membership.len();
            self.membership = members;
            self.changed = now;
            self.history.clear();
        }
        let age = now - self.changed;
        let topology = if self.merge && age < self.tuning.persistence {
            Topology::Regrouping
        } else if groups.len() == 1 {
            Topology::Together
        } else if age < self.tuning.persistence {
            Topology::Forming
        } else if groups.len() == 2 {
            Topology::Established
        } else {
            Topology::MultipleGroups
        };
        let gap_m = (groups.len() > 1).then(|| groups[0].rear_m - groups[1].front_m);
        if let Some(gap) = gap_m {
            if self
                .history
                .back()
                .is_none_or(|(t, _)| now - *t >= Duration::from_secs(1))
            {
                self.history.push_back((now, gap));
            }
            while self
                .history
                .get(1)
                .is_some_and(|(t, _)| now - *t >= self.tuning.window)
            {
                self.history.pop_front();
            }
        }
        let gap_rate_ms = self.history.front().and_then(|(t, g)| {
            (now - *t >= self.tuning.window)
                .then(|| (gap_m.unwrap() - g) / (now - *t).as_secs_f64())
        });
        let pursuit = match (gap_m, intent, gap_rate_ms) {
            (None, _, _) => Pursuit::NoSeparation,
            (_, ChaseIntent::None, _) => Pursuit::Unchallenged,
            (_, ChaseIntent::Manage, _) => Pursuit::Managed,
            (_, _, None) => Pursuit::Observing,
            (_, _, Some(r)) if r < -self.tuning.deadband => Pursuit::Closing,
            (_, _, Some(r)) if r > self.tuning.deadband => Pursuit::LosingGround,
            _ => Pursuit::Stable,
        };
        let view = Observation {
            topology,
            pursuit,
            groups,
            gap_m,
            gap_rate_ms,
        };
        self.last = Some((state.clone(), intent, view.clone()));
        Ok(view)
    }
}
