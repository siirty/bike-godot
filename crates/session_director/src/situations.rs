//! Player knowledge and pre-ride learning contracts. No live state or hidden opponent telemetry.
use serde::Serialize;
use std::collections::BTreeSet;

/// Public, qualitative reputation. These are priors, not current-state threat labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum KnownStrength {
    /// Sustained attacks and climbing.
    Sustained,
    /// Short finishing efforts.
    Sprint,
    /// Broad capability.
    AllRound,
    /// No reliable reputation available.
    Unknown,
}
/// The publicly communicated local objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlayerObjective {
    /// Remain in contention for the finish.
    ContestFinish,
    /// Help the team remain represented at the front.
    SupportTeam,
    /// Maintain contact, irrespective of who represents the team ahead.
    StayWithGroup,
}
/// A roster entry contains no exact CP, reserve, tactical intention or hidden role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KnownRider {
    id: String,
    name: String,
    team: Option<String>,
    strength: KnownStrength,
}
impl KnownRider {
    /// Construct a public identity, rejecting blank identifiers, names and team names.
    pub fn new(
        id: String,
        name: String,
        team: Option<String>,
        strength: KnownStrength,
    ) -> Result<Self, &'static str> {
        if id.trim().is_empty()
            || name.trim().is_empty()
            || team.as_ref().is_some_and(|t| t.trim().is_empty())
        {
            return Err("roster identity/name/team cannot be blank");
        }
        Ok(Self {
            id,
            name,
            team,
            strength,
        })
    }
    /// Stable identity shared with the simulation.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Display name that does not disclose the author's role.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Known team affiliation, if any.
    pub fn team(&self) -> Option<&str> {
        self.team.as_deref()
    }
    /// Known qualitative capability.
    pub fn strength(&self) -> KnownStrength {
        self.strength
    }
}
/// All persistent facts available to the reference rider; separate from the authored answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlayerKnowledge {
    player: String,
    objective: PlayerObjective,
    briefing: String,
    riders: Vec<KnownRider>,
}
impl PlayerKnowledge {
    /// Canonicalize the roster and reject duplicate/missing player identities and empty briefing.
    pub fn new(
        player: String,
        objective: PlayerObjective,
        briefing: String,
        mut riders: Vec<KnownRider>,
    ) -> Result<Self, &'static str> {
        if briefing.trim().is_empty() || !riders.iter().any(|r| r.id == player) {
            return Err("briefing and player identity are required");
        }
        let ids: BTreeSet<_> = riders.iter().map(|r| &r.id).collect();
        if ids.len() != riders.len() {
            return Err("duplicate roster identity");
        }
        riders.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self {
            player,
            objective,
            briefing,
            riders,
        })
    }
    /// Public athlete identity.
    pub fn player(&self) -> &str {
        &self.player
    }
    /// Public objective.
    pub fn objective(&self) -> PlayerObjective {
        self.objective
    }
    /// Essential pre-situation context, independent of coaching.
    pub fn briefing(&self) -> &str {
        &self.briefing
    }
    /// Known roster in stable order.
    pub fn riders(&self) -> &[KnownRider] {
        &self.riders
    }
    /// Lookup does not fabricate knowledge of unfamiliar riders.
    pub fn rider(&self, id: &str) -> Option<&KnownRider> {
        self.riders.iter().find(|r| r.id == id)
    }
    /// Unknown/unaffiliated riders are never assumed to be teammates.
    pub fn teammates(&self, a: &str, b: &str) -> bool {
        match (
            self.rider(a).and_then(KnownRider::team),
            self.rider(b).and_then(KnownRider::team),
        ) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }
}
/// Diagnostic learning objective; never sent into a local controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LearningIntent {
    /// Conserve while movement is safe to ignore.
    Conserve,
    /// Maintain useful group contact.
    HoldContact,
    /// Contribute a sustained pull.
    TakePull,
    /// Respond to a consequential acceleration.
    Respond,
    /// Contest a finish.
    Finish,
    /// Recover after the race.
    Cooldown,
}
/// A declared physiological envelope, not a controller input.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EffortEnvelope {
    lower_w: f64,
    upper_w: f64,
    minimum_share: f64,
}
impl EffortEnvelope {
    /// Reject unordered, negative and non-finite bands and invalid time shares.
    pub fn new(lower_w: f64, upper_w: f64, minimum_share: f64) -> Result<Self, &'static str> {
        if !lower_w.is_finite()
            || !upper_w.is_finite()
            || lower_w < 0.0
            || upper_w < lower_w
            || !minimum_share.is_finite()
            || !(0.0..=1.0).contains(&minimum_share)
        {
            return Err("invalid effort envelope");
        }
        Ok(Self {
            lower_w,
            upper_w,
            minimum_share,
        })
    }
    /// Inclusive watts band.
    pub fn contains(&self, watts: f64) -> bool {
        watts >= self.lower_w && watts <= self.upper_w
    }
    /// Minimum fraction of duration in band.
    pub fn minimum_share(&self) -> f64 {
        self.minimum_share
    }
}
/// Input specification for one authored situation. This is not an observed tactical verdict.
#[derive(Debug, Clone)]
pub struct SituationSpec {
    /// Stable lesson identity.
    pub id: String,
    /// Whole-second workout start.
    pub start_s: u32,
    /// Whole-second duration.
    pub duration_s: u32,
    /// Intended distinction, evaluator only.
    pub intent: LearningIntent,
    /// Evidence the author expects the player to see.
    pub cues: Vec<String>,
    /// Acceptable behavior descriptions; alternatives are explicit.
    pub responses: Vec<String>,
    /// Nominal response window relative to the first observed cue, in seconds.
    pub response_window_s: f64,
    /// Stimulus band.
    pub effort: EffortEnvelope,
    /// Plausible local consequence of following, missing or unnecessarily covering the move.
    pub consequence: String,
    /// Conditions under which the next lesson remains available.
    pub exit: String,
}
/// Validated pre-ride situation. Private fields prevent unchecked contracts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LearningSituation {
    id: String,
    start_s: u32,
    duration_s: u32,
    intent: LearningIntent,
    cues: Vec<String>,
    responses: Vec<String>,
    response_window_s: f64,
    effort: EffortEnvelope,
    exit: String,
    consequence: String,
}
impl LearningSituation {
    /// Validate duration, response window and required author explanations.
    pub fn new(s: SituationSpec) -> Result<Self, &'static str> {
        if s.id.trim().is_empty()
            || s.duration_s == 0
            || s.start_s.checked_add(s.duration_s).is_none()
            || !s.response_window_s.is_finite()
            || s.response_window_s <= 0.0
            || s.response_window_s > f64::from(s.duration_s)
            || s.cues.is_empty()
            || s.responses.is_empty()
            || s.cues
                .iter()
                .chain(&s.responses)
                .any(|x| x.trim().is_empty())
            || s.exit.trim().is_empty()
            || s.consequence.trim().is_empty()
        {
            return Err("invalid situation identity, timing or explanation");
        }
        Ok(Self {
            id: s.id,
            start_s: s.start_s,
            duration_s: s.duration_s,
            intent: s.intent,
            cues: s.cues,
            responses: s.responses,
            response_window_s: s.response_window_s,
            effort: s.effort,
            exit: s.exit,
            consequence: s.consequence,
        })
    }
    /// Scheduled start, independent of actual cue onset.
    pub fn start_s(&self) -> u32 {
        self.start_s
    }
    /// Scheduled duration.
    pub fn duration_s(&self) -> u32 {
        self.duration_s
    }
    /// Evaluator-only teaching intent.
    pub fn intent(&self) -> LearningIntent {
        self.intent
    }
    /// Workout envelope, evaluator only.
    pub fn effort(&self) -> &EffortEnvelope {
        &self.effort
    }
    /// Stable situation identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Hypothesized cue-relative response window, not an empirical constant.
    pub fn response_window_s(&self) -> f64 {
        self.response_window_s
    }
}
/// Versioned, contiguous authored sequence and its public context. No opponent trajectories.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LearningPlan {
    version: u32,
    knowledge: PlayerKnowledge,
    situations: Vec<LearningSituation>,
}
impl LearningPlan {
    /// Reject empty sequences, duplicate IDs, gaps and overlaps. Continuity of actual racing must
    /// still be checked in execution; contiguity alone is not physical reachability.
    pub fn new(
        knowledge: PlayerKnowledge,
        situations: Vec<LearningSituation>,
    ) -> Result<Self, &'static str> {
        if situations.is_empty()
            || situations[0].start_s != 0
            || situations
                .windows(2)
                .any(|w| w[0].start_s + w[0].duration_s != w[1].start_s)
            || situations
                .iter()
                .map(|s| &s.id)
                .collect::<BTreeSet<_>>()
                .len()
                != situations.len()
        {
            return Err("situation sequence must be nonempty, contiguous and uniquely identified");
        }
        Ok(Self {
            version: 1,
            knowledge,
            situations,
        })
    }
    /// Public facts, safe to supply to the reference reader.
    pub fn knowledge(&self) -> &PlayerKnowledge {
        &self.knowledge
    }
    /// Authored contracts, not safe to supply as tactical perception.
    pub fn situations(&self) -> &[LearningSituation] {
        &self.situations
    }
}
