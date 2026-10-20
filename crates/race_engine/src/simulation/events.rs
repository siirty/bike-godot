//! The kernel's event vocabulary, and how a step's events are derived from the pre-step and
//! post-step states.
//!
//! Events are a *report* about a step, derived only from the two states and the configuration.
//! They are never an input, never stored in [`RaceState`], and never in a snapshot: a resumed race
//! re-derives exactly the events an uninterrupted one would have produced.

use std::cmp::Ordering;
use std::time::Duration;

use crate::simulation::error::{RaceSimulationError, Result, SimulationValidationCode as C};
use crate::{RaceSimulationConfig, RaceState, RiderId, RiderState};

/// What happened. The declaration order is the within-step emission order, and part of the
/// contract: changing it changes every recorded event sequence.
///
/// Deliberately not `#[non_exhaustive]`: consumers match every kind, and a new kind is meant to
/// break those matches.
#[derive(Debug, Clone, PartialEq)]
pub enum RaceEventKind {
    /// `rider` moved from one lateral slot to another.
    SlotChanged {
        /// Who moved.
        rider: RiderId,
        /// The slot before the step.
        from_slot: usize,
        /// The slot after the step.
        to_slot: usize,
    },
    /// `rider` no longer drafts `target`: the wheel went out of range, or the rider chose
    /// another.
    WheelLost {
        /// The drafting rider.
        rider: RiderId,
        /// The wheel it had.
        target: RiderId,
    },
    /// `rider` now drafts `target`.
    WheelTaken {
        /// The drafting rider.
        rider: RiderId,
        /// The wheel it took.
        target: RiderId,
    },
    /// `rider` was strictly behind `passed` before the step and is strictly ahead after it.
    /// Exactly equal distances are neither ahead nor behind, so passing through an exact tie is
    /// not reported.
    Overtook {
        /// The rider who moved ahead.
        rider: RiderId,
        /// The rider now behind.
        passed: RiderId,
    },
    /// `rider` reached the course's explicit length this step, finishing in `place` (1-based).
    Finished {
        /// Who finished.
        rider: RiderId,
        /// Finishing position across the whole race.
        place: u32,
    },
}

impl RaceEventKind {
    fn ordinal(&self) -> u8 {
        match self {
            Self::SlotChanged { .. } => 0,
            Self::WheelLost { .. } => 1,
            Self::WheelTaken { .. } => 2,
            Self::Overtook { .. } => 3,
            Self::Finished { .. } => 4,
        }
    }

    /// The rider the event is about, then the other rider if there is one.
    fn roles(&self) -> (&RiderId, Option<&RiderId>) {
        match self {
            Self::SlotChanged { rider, .. } | Self::Finished { rider, .. } => (rider, None),
            Self::WheelLost { rider, target } | Self::WheelTaken { rider, target } => {
                (rider, Some(target))
            }
            Self::Overtook { rider, passed } => (rider, Some(passed)),
        }
    }
}

/// One validated event: when it happened (the post-step simulation time) and what happened.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceEvent {
    simulation_time: Duration,
    kind: RaceEventKind,
}

impl RaceEvent {
    /// Validates the kind: two-rider events name two different riders, a slot change changes the
    /// slot, and places start at 1.
    pub fn new(simulation_time: Duration, kind: RaceEventKind) -> Result<Self> {
        if let (rider, Some(other)) = kind.roles() {
            if rider == other {
                return Err(RaceSimulationError::validation(
                    C::DuplicateRiderId,
                    format!("an event cannot involve rider \"{rider}\" twice"),
                    "involvedRiderIds",
                ));
            }
        }
        match &kind {
            RaceEventKind::SlotChanged {
                from_slot, to_slot, ..
            } if from_slot == to_slot => {
                return Err(RaceSimulationError::validation(
                    C::OutOfRange,
                    format!("a slot change must change the slot, got {from_slot} → {to_slot}"),
                    "toSlot",
                ));
            }
            RaceEventKind::Finished { place: 0, .. } => {
                return Err(RaceSimulationError::validation(
                    C::NotPositive,
                    "finishing places start at 1",
                    "place",
                ));
            }
            _ => {}
        }
        Ok(Self {
            simulation_time,
            kind,
        })
    }

    /// The post-step simulation time of the step that produced this event.
    pub fn simulation_time(&self) -> Duration {
        self.simulation_time
    }

    /// What happened.
    pub fn kind(&self) -> &RaceEventKind {
        &self.kind
    }

    /// Every rider involved, in canonical order.
    pub fn involved_rider_ids(&self) -> Vec<&RiderId> {
        let (rider, other) = self.kind.roles();
        let mut ids: Vec<&RiderId> = std::iter::once(rider).chain(other).collect();
        ids.sort();
        ids
    }

    /// The within-step order: kind (declaration order); then finishers by place, and every other
    /// kind by the primary rider, then the other rider, canonically. Independent of the order
    /// detection happened to run in.
    fn emission_cmp(&self, other: &Self) -> Ordering {
        let by_kind = self.kind.ordinal().cmp(&other.kind.ordinal());
        if let (
            RaceEventKind::Finished { place: p, .. },
            RaceEventKind::Finished { place: q, .. },
        ) = (&self.kind, &other.kind)
        {
            return by_kind.then(p.cmp(q));
        }
        let (a, a2) = self.kind.roles();
        let (b, b2) = other.kind.roles();
        by_kind.then_with(|| a.cmp(b)).then_with(|| a2.cmp(&b2))
    }
}

/// The events of the step `pre → post`, in emission order. Both states are canonical and hold the
/// same riders (the kernel guarantees it), so riders pair up by index.
pub(crate) fn detect_events(
    config: &RaceSimulationConfig,
    pre: &RaceState,
    post: &RaceState,
) -> Result<Vec<RaceEvent>> {
    let time = post.simulation_time();
    let pairs: Vec<(&RiderState, &RiderState)> = pre.riders().iter().zip(post.riders()).collect();
    let mut events = Vec::new();

    for (before, after) in &pairs {
        let rider = after.id();
        if before.lateral_slot() != after.lateral_slot() {
            events.push(RaceEvent::new(
                time,
                RaceEventKind::SlotChanged {
                    rider: rider.clone(),
                    from_slot: before.lateral_slot(),
                    to_slot: after.lateral_slot(),
                },
            )?);
        }
        match (before.draft_target_id(), after.draft_target_id()) {
            (Some(old), new) if new != Some(old) => events.push(RaceEvent::new(
                time,
                RaceEventKind::WheelLost {
                    rider: rider.clone(),
                    target: old.clone(),
                },
            )?),
            _ => {}
        }
        match (before.draft_target_id(), after.draft_target_id()) {
            (old, Some(new)) if old != Some(new) => events.push(RaceEvent::new(
                time,
                RaceEventKind::WheelTaken {
                    rider: rider.clone(),
                    target: new.clone(),
                },
            )?),
            _ => {}
        }
    }

    for (a_before, a_after) in &pairs {
        for (b_before, b_after) in &pairs {
            if a_before.distance_m() < b_before.distance_m()
                && a_after.distance_m() > b_after.distance_m()
            {
                events.push(RaceEvent::new(
                    time,
                    RaceEventKind::Overtook {
                        rider: a_after.id().clone(),
                        passed: b_after.id().clone(),
                    },
                )?);
            }
        }
    }

    if let Some(finish_m) = config.terrain().explicit_length_m() {
        let already = pairs
            .iter()
            .filter(|(before, _)| before.distance_m() >= finish_m)
            .count();
        // Same-step finishers are ordered by when they crossed inside the step (linear in the
        // step's distance), then canonically.
        let mut crossing: Vec<(f64, &RiderId)> = pairs
            .iter()
            .filter(|(b, a)| b.distance_m() < finish_m && a.distance_m() >= finish_m)
            .map(|(b, a)| {
                let fraction = (finish_m - b.distance_m()) / (a.distance_m() - b.distance_m());
                (fraction, a.id())
            })
            .collect();
        crossing.sort_by(|x, y| x.0.total_cmp(&y.0).then_with(|| x.1.cmp(y.1)));
        for (i, (_, rider)) in crossing.into_iter().enumerate() {
            events.push(RaceEvent::new(
                time,
                RaceEventKind::Finished {
                    rider: rider.clone(),
                    place: (already + i + 1) as u32,
                },
            )?);
        }
    }

    events.sort_by(RaceEvent::emission_cmp);
    Ok(events)
}
