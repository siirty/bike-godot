//! A transparent reference reader using public knowledge and observable motion only.
//! Its heuristics are synthetic and configurable; it is not a validated human recognizer.
use crate::{RaceSimulationConfig, RaceState};
use serde::Serialize;
use session_director::situations::{KnownStrength, PlayerKnowledge, PlayerObjective};
use std::collections::BTreeSet;

/// Observable motion; no power, physiological reserve, future inputs or lesson labels.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RiderMotion {
    /// Identity may be unfamiliar to the player.
    pub id: String,
    /// Relative longitudinal distance, m.
    pub gap_m: f64,
    /// Speed relative to athlete, m/s.
    pub relative_speed_ms: f64,
    /// Visible lateral file occupied by the rider, not a promised draft target.
    pub lateral_slot: usize,
}
/// Sanitized perception input. Constructed from the same pre-step state as the kernel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublicRaceView {
    /// Rider motions in canonical order, excluding the athlete.
    pub riders: Vec<RiderMotion>,
    /// Current observed road grade.
    pub grade: f64,
    /// Distance to the finish, if the course has one; negative after crossing.
    pub finish_remaining_m: Option<f64>,
}
impl PublicRaceView {
    /// Sanitize kernel state for the public reader, validating config/state and athlete identity.
    pub fn observe(
        config: &RaceSimulationConfig,
        state: &RaceState,
        player: &str,
    ) -> Result<Self, String> {
        config
            .validate_state(state, "perception")
            .map_err(|e| e.to_string())?;
        let h = state
            .riders()
            .iter()
            .find(|r| r.id().as_str() == player)
            .ok_or("unknown observation player")?;
        Ok(Self {
            riders: state
                .riders()
                .iter()
                .filter(|r| r.id() != h.id())
                .map(|r| RiderMotion {
                    id: r.id().as_str().into(),
                    gap_m: r.distance_m() - h.distance_m(),
                    relative_speed_ms: r.speed_ms() - h.speed_ms(),
                    lateral_slot: r.lateral_slot(),
                })
                .collect(),
            grade: config.terrain().grade_at(h.distance_m()),
            finish_remaining_m: config
                .terrain()
                .explicit_length_m()
                .map(|d| d - h.distance_m()),
        })
    }
}
/// Reference interpretation, distinct from author intent and human recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MoveDecision {
    /// Remain economical with the nearby group.
    Shelter,
    /// Teammate representation makes an immediate chase unnecessary for the stated objective.
    LetOthersWork,
    /// Known sustained opposition is opening a relevant gap.
    Respond,
    /// Short-range finish opportunity.
    Sprint,
    /// A move is visible but reputation/context is insufficient for a confident instruction.
    Monitor,
    /// Athlete has crossed the finish.
    Finished,
}
/// A classification includes the observed target and explicit public reasons, never hidden state.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MoveRead {
    /// Reference interpretation, not a human assessment score.
    pub decision: MoveDecision,
    /// Target identity, if an actionable/observable target exists.
    pub target: Option<String>,
    /// Public evidence explaining this heuristic.
    pub reasons: Vec<String>,
}
/// Tunable engineering assumptions, not universal cycling reaction thresholds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReaderSettings {
    /// Minimum gap indicating a move outside close shelter.
    pub move_gap_m: f64,
    /// Maximum local visibility/action range.
    pub visibility_m: f64,
    /// A small gap only counts as opening if relative speed exceeds this threshold.
    pub opening_speed_ms: f64,
    /// A persistent larger gap also warrants attention without current acceleration.
    pub established_gap_m: f64,
    /// Finishing context in which a final effort is considered.
    pub sprint_distance_m: f64,
}
impl Default for ReaderSettings {
    fn default() -> Self {
        Self {
            move_gap_m: 8.0,
            visibility_m: 100.0,
            opening_speed_ms: 0.3,
            established_gap_m: 20.0,
            sprint_distance_m: 180.0,
        }
    }
}
impl ReaderSettings {
    /// Reject nonfinite, negative or inconsistent observation bounds.
    pub fn validate(&self) -> Result<(), &'static str> {
        if [
            self.move_gap_m,
            self.visibility_m,
            self.opening_speed_ms,
            self.established_gap_m,
            self.sprint_distance_m,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
            || self.established_gap_m <= self.move_gap_m
            || self.visibility_m < self.established_gap_m
        {
            return Err("invalid reader settings");
        }
        Ok(())
    }
}
/// Classify from the sanitized view. No schedule, authored answer, reserve or workout can enter.
pub fn read_move(
    knowledge: &PlayerKnowledge,
    view: &PublicRaceView,
    settings: ReaderSettings,
) -> Result<MoveRead, &'static str> {
    settings.validate()?;
    if view
        .riders
        .iter()
        .map(|r| &r.id)
        .collect::<BTreeSet<_>>()
        .len()
        != view.riders.len()
        || view.riders.iter().any(|r| r.id == knowledge.player())
        || !view.grade.is_finite()
        || view.finish_remaining_m.is_some_and(|x| !x.is_finite())
        || view.riders.iter().any(|r| {
            r.id.trim().is_empty() || !r.gap_m.is_finite() || !r.relative_speed_ms.is_finite()
        })
    {
        return Err("invalid public observation");
    }
    let result = |decision, target: Option<&str>, reason: &str| MoveRead {
        decision,
        target: target.map(str::to_owned),
        reasons: vec![reason.into()],
    };
    if view.finish_remaining_m.is_some_and(|d| d <= 0.0) {
        return Ok(result(
            MoveDecision::Finished,
            None,
            "Finish line crossed; race formation is no longer required.",
        ));
    }
    let near: Vec<_> = view
        .riders
        .iter()
        .filter(|r| r.gap_m.abs() <= settings.visibility_m)
        .collect();
    let nearest = near
        .iter()
        .filter(|r| r.gap_m > 0.0)
        .min_by(|a, b| a.gap_m.total_cmp(&b.gap_m).then(a.id.cmp(&b.id)))
        .map(|r| r.id.as_str());
    if knowledge.objective() == PlayerObjective::ContestFinish
        && view
            .finish_remaining_m
            .is_some_and(|d| d <= settings.sprint_distance_m)
    {
        return Ok(result(
            MoveDecision::Sprint,
            nearest,
            "The visible finish is close and the declared objective is to contest it.",
        ));
    }
    let moving: Vec<_> = near
        .iter()
        .copied()
        .filter(|r| {
            r.gap_m > settings.move_gap_m
                && (r.relative_speed_ms > settings.opening_speed_ms
                    || r.gap_m > settings.established_gap_m)
        })
        .collect();
    let threat = moving
        .iter()
        .filter(|r| {
            (knowledge.objective() == PlayerObjective::StayWithGroup
                || !knowledge.teammates(knowledge.player(), &r.id))
                && knowledge.rider(&r.id).is_some_and(|p| {
                    matches!(
                        p.strength(),
                        KnownStrength::Sustained | KnownStrength::AllRound
                    ) || r.gap_m > 2.0 * settings.established_gap_m
                })
        })
        .min_by(|a, b| a.gap_m.total_cmp(&b.gap_m).then(a.id.cmp(&b.id)));
    if let Some(r) = threat {
        let represented = near.iter().any(|ally| {
            knowledge.teammates(knowledge.player(), &ally.id)
                && (ally.gap_m - r.gap_m).abs() <= 8.0
                && ally.relative_speed_ms >= r.relative_speed_ms - settings.opening_speed_ms
        });
        if represented && knowledge.objective() != PlayerObjective::StayWithGroup {
            return Ok(result(MoveDecision::LetOthersWork,nearest,"Known opposition is ahead, but a teammate is alongside the move; conserve with the group for this objective."));
        }
        return Ok(result(MoveDecision::Respond,Some(&r.id),"Known opposition is opening or holding a gap; an established large gap can override sprint-only reputation. No teammate maintains coverage for this objective."));
    }
    if let Some(r) = moving
        .iter()
        .min_by(|a, b| a.gap_m.total_cmp(&b.gap_m).then(a.id.cmp(&b.id)))
    {
        if knowledge.teammates(knowledge.player(), &r.id)
            && knowledge.objective() != PlayerObjective::StayWithGroup
        {
            return Ok(result(MoveDecision::LetOthersWork,nearest,"A teammate is represented ahead; this objective does not require an immediate chase."));
        }
        return Ok(result(MoveDecision::Monitor,Some(&r.id),"A move is visible; available reputation does not establish that it must be covered. Keep observing its gap and company."));
    }
    Ok(result(MoveDecision::Shelter,nearest,"No local opening move meets this reference reader's threshold; remain economical and keep observing."))
}
