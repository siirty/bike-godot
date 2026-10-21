//! Fixture authoring and independent diagnostics; author intent never enters perception.
use crate::{Error, Phase, Plan, Run, NAMES};
use race_engine::perception::{MoveDecision, MoveRead};
use serde::Serialize;
use session_director::situations::*;

/// Build a public briefing and a validated situation sequence for the engineering fixture.
pub fn author(phases: &[Phase], ftp: f64) -> Result<LearningPlan, Error> {
    let roster = [
        ("You", "Green", KnownStrength::AllRound),
        ("Alex", "Blue", KnownStrength::Sprint),
        ("Jo", "Green", KnownStrength::Sustained),
        ("Morgan", "Blue", KnownStrength::Sustained),
        ("Sam", "Gold", KnownStrength::AllRound),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (name, team, strength))| {
        KnownRider::new(NAMES[i].into(), name.into(), Some(team.into()), strength)
    })
    .collect::<Result<Vec<_>, _>>()?;
    let knowledge=PlayerKnowledge::new(NAMES[0].into(),PlayerObjective::ContestFinish,
        "Stay in contention for the finish. You and Jo ride for Green; Alex and Morgan for Blue; Sam for Gold. Jo and Morgan are known for sustained efforts, Alex for a fast finish. Short climbs and descents lead to a flat finish. Synthetic reputations and capabilities; no calibrated race class.".into(),roster)?;
    let situations=phases.iter().enumerate().map(|(i,p)|{
        let intent=if i==phases.len()-1 {LearningIntent::Cooldown}else{match p.zone{1=>LearningIntent::Conserve,2=>LearningIntent::HoldContact,3=>LearningIntent::TakePull,7=>LearningIntent::Finish,_=>LearningIntent::Respond}};
        let cue=match intent{
            LearningIntent::Conserve=>"Observe who moves, the gap and whether others cover it; positional movement alone is not an attack.",
            LearningIntent::HoldContact=>"Observe group speed and available wheels, not one fixed rider.",
            LearningIntent::TakePull=>"Inspect rotation and team work; a public pull-duty signal is not implemented in this fixture.",
            LearningIntent::Respond=>"Observe sustained opposition opening a gap, its company and the road ahead.",
            LearningIntent::Finish=>"Observe the approaching finish, position and relative speed.",
            LearningIntent::Cooldown=>"After finishing, ease off; no race formation obligation remains.",
        };
        LearningSituation::new(SituationSpec{id:format!("fixture-{i}"),start_s:p.start,duration_s:p.duration,intent,cues:vec![cue.into()],responses:vec![p.action.clone(),"Other wheels/formations are acceptable if effort and objective remain available.".into()],response_window_s:f64::from(p.duration.min(8)),effort:EffortEnvelope::new((p.intensity*ftp-0.1*ftp).max(0.0),p.intensity*ftp+0.1*ftp,0.8)?,consequence:"Missed moves can leave persistent gaps; unnecessary covering spends actual energy.".into(),exit:"Remain in contention with a physically plausible transition; no automatic rescue.".into()})
    }).collect::<Result<Vec<_>,_>>()?;
    Ok(LearningPlan::new(knowledge, situations)?)
}
/// Verdicts explicitly distinguish engineering evidence from unperformed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    /// Meets a provisional numerical diagnostic.
    MeetsDiagnostic,
    /// Misses a provisional numerical diagnostic.
    MissedDiagnostic,
    /// Independent human/empirical review is still required.
    NeedsReview,
    /// Not a race requirement here.
    NotApplicable,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct LearningAcc {
    pub ticks: u32,
    pub band: u32,
    pub opportunity: u32,
    pub cue_at: Option<f64>,
    pub response_at: Option<f64>,
}
impl LearningAcc {
    pub fn observe(
        &mut self,
        p: &LearningSituation,
        time: f64,
        delivered: f64,
        contender: bool,
        read: &MoveRead,
    ) {
        self.ticks += 1;
        self.band += u32::from(p.effort().contains(delivered));
        self.opportunity += u32::from(contender);
        if read.decision == MoveDecision::Respond && self.cue_at.is_none() {
            self.cue_at = Some(time);
        }
    }
}
/// Independent per-situation diagnostics measured at 20 Hz.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SituationAudit {
    /// Original phase index.
    pub phase: usize,
    /// Share of actual delivered power within the original authored band.
    pub effort_share: f64,
    /// Workload verdict does not depend on formation or winning.
    pub workout: Verdict,
    /// Share within a provisional contention window, allowing being ahead.
    pub opportunity_share: f64,
    /// Continuity proxy, not a future-reachability proof.
    pub continuity: Verdict,
    /// Human recognition remains unassessed by this software.
    pub recognition: Verdict,
    /// First reference-reader cue observed in the phase, absolute seconds.
    pub first_cue_s: Option<f64>,
    /// Delay until a >5 W rise in requested athlete power after the cue.
    pub response_delay_s: Option<f64>,
    /// Provisional cue-relative timing diagnostic.
    pub timing: Verdict,
    /// Limitations and explanations.
    pub notes: Vec<String>,
}
/// Recorded physical evidence for a rider's path to an outcome. No winner/archetype veto.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RiderWitness {
    /// Kernel identity; display uses public roster names.
    pub rider: String,
    /// Physiology-delivered work to the finish or replay horizon, in kJ.
    pub work_kj: f64,
    /// Peak 50 ms mean delivered watts.
    pub peak_w: f64,
    /// Minimum actual W′ fraction.
    pub min_reserve: f64,
    /// Largest deficit to the front while this rider climbs above 1% grade.
    pub max_climb_deficit_m: f64,
    /// Actual finish event, if any.
    pub finish_s: Option<f64>,
    /// Actual kernel place, if any.
    pub place: Option<u32>,
}
/// Mechanical evidence and empirical plausibility are different claims.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ResultAudit {
    /// Review pending, never promoted from nominal fitting or a desired winner.
    pub plausibility: Verdict,
    /// Configuration/state validation ran successfully at every step.
    pub mechanical_integrity_checked: bool,
    /// Path evidence for all riders.
    pub riders: Vec<RiderWitness>,
    /// Explicit evidence limits.
    pub notes: Vec<String>,
}
pub(crate) fn audit(plan: &Plan, acc: &[LearningAcc], run: &Run) -> Vec<SituationAudit> {
    plan.learning.situations().iter().zip(acc).enumerate().map(|(i,(p,a))|{
        let n=f64::from(a.ticks).max(1.0);let effort_share=f64::from(a.band)/n;let opportunity_share=f64::from(a.opportunity)/n;
        let racing=p.intent()!=LearningIntent::Cooldown&&!run.finish_s.is_some_and(|t|t<=f64::from(p.start_s()));
        let response_delay_s=a.cue_at.zip(a.response_at).map(|(c,r)|(r-c).max(0.0));
        let timing=if p.intent()!=LearningIntent::Respond||!racing{Verdict::NotApplicable}else if let Some(delay)=response_delay_s{if delay<=p.response_window_s(){Verdict::MeetsDiagnostic}else{Verdict::MissedDiagnostic}}else if a.cue_at.is_some(){Verdict::MissedDiagnostic}else{Verdict::NeedsReview};
        SituationAudit{phase:i,effort_share,workout:if effort_share>=p.effort().minimum_share(){Verdict::MeetsDiagnostic}else{Verdict::MissedDiagnostic},opportunity_share,continuity:if !racing{Verdict::NotApplicable}else if opportunity_share>=0.5{Verdict::MeetsDiagnostic}else{Verdict::MissedDiagnostic},recognition:Verdict::NeedsReview,first_cue_s:a.cue_at,response_delay_s,timing,notes:vec!["Recognition is unassessed; the reference reader is not evidence of human learning.".into(),if p.intent()==LearningIntent::TakePull{"Public pull-duty communication is missing; this lesson's readability is unproven.".into()}else{"Reader, response window and 60 m contention bound are provisional engineering assumptions.".into()},"Contention accepts other formations/being ahead; it does not prove future effort-equivalent reachability.".into()]}
    }).collect()
}
