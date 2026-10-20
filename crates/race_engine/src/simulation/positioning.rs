//! Resolving positioning and drafting intentions into one step's physics inputs, always from one
//! immutable pre-step snapshot, so processing order can never create a physical advantage.

use crate::{BasicDraftingConfig, RaceInputFrame, RaceSimulationConfig, RaceState, RiderId};

/// Each rider's slot this step: the request if any (accepted; soft occupancy is intentional in
/// Slice 4), otherwise the current slot. Inputs were validated against the config already.
pub fn resolve_lateral_slots(pre_step: &RaceState, inputs: &RaceInputFrame) -> Vec<usize> {
    pre_step
        .riders()
        .iter()
        .map(|r| {
            inputs
                .input_for(r.id())
                .and_then(|i| i.lateral_slot())
                .unwrap_or(r.lateral_slot())
        })
        .collect()
}

/// Each rider's working draft target: requested (if eligible) → current (if still eligible) →
/// the nearest eligible rider ahead, ties to the lower canonical ID. All `None` without a
/// drafting model.
pub fn select_draft_targets(
    config: &RaceSimulationConfig,
    pre_step: &RaceState,
    resolved_slots: &[usize],
    inputs: &RaceInputFrame,
) -> Vec<Option<RiderId>> {
    let riders = pre_step.riders();
    let Some(drafting) = config.drafting() else {
        return vec![None; riders.len()];
    };
    (0..riders.len())
        .map(|i| {
            let rider = &riders[i];
            let pick = |candidate: Option<&RiderId>| -> Option<RiderId> {
                let candidate = candidate?;
                if candidate == rider.id() {
                    return None;
                }
                let j = index_of(pre_step, candidate)?;
                let gap = riders[j].distance_m() - rider.distance_m();
                let offset = resolved_slots[j].abs_diff(resolved_slots[i]);
                is_eligible(drafting, gap, offset).then(|| candidate.clone())
            };
            let requested = inputs
                .input_for(rider.id())
                .and_then(|x| x.draft_target_id());
            if let Some(t) = pick(requested) {
                return Some(t);
            }
            if let Some(t) = pick(rider.draft_target_id()) {
                return Some(t);
            }
            let mut best: Option<(usize, f64)> = None;
            for (j, other) in riders.iter().enumerate() {
                if j == i {
                    continue;
                }
                let gap = other.distance_m() - rider.distance_m();
                let offset = resolved_slots[j].abs_diff(resolved_slots[i]);
                if !is_eligible(drafting, gap, offset) {
                    continue;
                }
                if best.is_none_or(|(_, best_gap)| gap < best_gap) {
                    best = Some((j, gap));
                }
            }
            best.map(|(j, _)| riders[j].id().clone())
        })
        .collect()
}

/// Every rider's aerodynamic drag multiplier from the shared pre-step geometry (exactly 1.0
/// without a drafting model or a target).
pub fn compute_drag_multipliers(
    config: &RaceSimulationConfig,
    pre_step: &RaceState,
    resolved_slots: &[usize],
    resolved_targets: &[Option<RiderId>],
) -> Vec<f64> {
    let riders = pre_step.riders();
    let Some(drafting) = config.drafting() else {
        return vec![1.0; riders.len()];
    };
    (0..riders.len())
        .map(|i| match &resolved_targets[i] {
            None => 1.0,
            Some(target) => {
                let j = index_of(pre_step, target).expect("selected from this state");
                let gap = riders[j].distance_m() - riders[i].distance_m();
                let offset = resolved_slots[j].abs_diff(resolved_slots[i]);
                drag_multiplier(drafting, gap, offset)
            }
        })
        .collect()
}

/// Keeps each working target only if it is still eligible under post-step distances, so the
/// next state describes an actually available wheel.
pub fn retain_eligible_targets(
    config: &RaceSimulationConfig,
    post_step: &RaceState,
    resolved_slots: &[usize],
    resolved_targets: &[Option<RiderId>],
) -> Vec<Option<RiderId>> {
    let riders = post_step.riders();
    let Some(drafting) = config.drafting() else {
        return vec![None; riders.len()];
    };
    (0..riders.len())
        .map(|i| {
            let target = resolved_targets[i].as_ref()?;
            let j = index_of(post_step, target).expect("same rider set");
            let gap = riders[j].distance_m() - riders[i].distance_m();
            let offset = resolved_slots[j].abs_diff(resolved_slots[i]);
            is_eligible(drafting, gap, offset).then(|| target.clone())
        })
        .collect()
}

fn index_of(state: &RaceState, id: &RiderId) -> Option<usize> {
    state.riders().binary_search_by(|r| r.id().cmp(id)).ok()
}

fn is_eligible(drafting: &BasicDraftingConfig, gap: f64, offset: usize) -> bool {
    gap > 0.0 && gap < drafting.max_gap_m() && offset < drafting.slot_count()
}

fn drag_multiplier(drafting: &BasicDraftingConfig, gap: f64, offset: usize) -> f64 {
    if !is_eligible(drafting, gap, offset) {
        return 1.0;
    }
    let near_wheel = drafting.multiplier_for(offset);
    let gap_fraction = gap / drafting.max_gap_m();
    let raw = near_wheel + (1.0 - near_wheel) * gap_fraction;
    raw.clamp(near_wheel, 1.0)
}
