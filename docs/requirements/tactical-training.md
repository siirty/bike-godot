# Tactical training through race situations

Accepted product requirements, 21 October 2026. These supersede the earlier equal-priority
full-race/game framing. [ADR 0007](../decisions/0007-tactical-training-core.md) records the decision;
[physics-design.md](../architecture/physics-design.md) owns orchestration and simulation boundaries.
These requirements describe intended behavior, not features already implemented.

## Purpose and scope

The trainer turns an immutable structured workout into connected, credible race situations that
teach the athlete **when to commit power, when to conserve, and how to classify moves**. The
workout defines the physiological stimulus. Tactical interpretation gives that effort a reason.
Reproducing a full historical race, a large peloton or a comprehensive racing career is not the
product objective. Nevertheless, the situations, their continuity and any resulting race finish
must be plausible together. SIM-only control and actual athlete power remain authoritative.

## Requirements

### TT-01: Define a tactical learning situation

Each situation must specify, conceptually (no serialization schema is settled here):

- the associated workout step/block and required effort/recovery envelope;
- the player's local objective and role;
- the initial course and field context, including relevant rider capabilities and affiliations;
- what the player knows beforehand, what becomes observable, and what remains uncertain;
- the move classification or tactical distinction to learn;
- observable cues, a response opportunity/window and acceptable responses or alternatives;
- the physical action that makes the prescribed effort useful, including duration;
- plausible local consequences of timely, late, excessive, insufficient or unnecessary effort;
- acceptable exit conditions and the connection to remaining workout situations;
- evidence and assumptions supporting the situation's tactical and physical plausibility.

An authored label such as “threatening” is a hypothesis to validate, not evidence. A situation is
not identified solely by a phase name, a rider ID, exact geometry or a target-watt trace.

### TT-02: Make the effort tactically meaningful

Appropriate execution of the intended action must offer the planned stimulus within explicit,
validated tolerances. Delivering prescribed watts alone does not establish that racing induces
the effort. An action-following reference policy must execute from observable race cues without
access to the workout watts; compare its actual demand and delivered effort with the contract.
Technique-defined steps may retain explicit coaching where race geometry cannot convey them.

### TT-03: Teach timing and classification

Cues must provide a learnable response window before a consequential action is lost. Validate
recognition, timing and effort execution separately. Immediate response is not always correct:
following, waiting, letting another rider chase and conserving can be legitimate answers.
Consequences arise from ordinary physics, fatigue and tactics, not a punishment for missing a UI
prompt. Exact reaction windows remain to be calibrated; no universal one- or three-second rule
is accepted.

### TT-04: Keep background movement safe to ignore

During planned recovery and transitions, incidental movement must not require an unplanned
race-saving effort under the situation's stated conditions and reference bounds. Its lower threat
must be legible from constellation, timing, terrain, rider identity/capability and others' actions.
Do not use flashing warnings or a threat badge as the necessary distinguishing signal.
“Safe to ignore” does not mean that following can never improve position or change a later placing.
Validate the cost of ignoring and the benefit/cost of covering against the player's stated objective.
Do not mark every move by a “weak” rider harmless regardless of current context.

### TT-05: Give the player the information required to decide

A judgment may depend only on information communicated beforehand or reasonably observable in the
scene. Provide the player objective/role, recognizable relevant riders, team membership, qualitative
known strengths, pertinent course/finish context and observable recent actions. If identity or
team membership changes the correct response, it is part of the minimum lesson scope.

Use consistent visible identities/jerseys and an accessible roster or equivalent briefing. Known
strengths are useful priors, not guarantees. Private AI intent and exact remaining reserve are not
player knowledge. Unknown riders/information are permitted when uncertainty is explicit and the
lesson remains fair; do not grade against unavailable decisive facts. No full start list or exact
opponent FTP display is required. Persistent familiar riders can support learning; durable roster
storage, its UI layout and roster size remain implementation choices.

### TT-06: Separate context, optional coaching and the answer

Learning to read the scene must remain possible with tactical instructions and workout-target
hints hidden. Essential context remains available independently of those aids. In assessment,
classification must not be revealed by hidden author labels, predetermined avatar identities or
explicit “respond now” instructions. Optional guided practice and post-situation explanation may
show the intended interpretation. A minimal HUD is not a reason to remove decision-critical facts.

### TT-07: Plan connected opportunities before the ride

Use deterministic pre-ride planning of situation sequence, terrain, field and bounded action/role
plans as the initial approach. Terrain and capability remain fixed during a session. Generation
must check that all remaining prescribed steps can be encountered in sequence after accepted
execution, including reasonable variation within declared reference bounds. This is not a promise
that arbitrary rider behavior leaves every later opportunity available.

Ordinary rider mistakes retain their consequences. A general live Scene Planner, automatic
neutralization or catch-up regroup is not a requirement for the initial approach. Local opponents
may still react plausibly to observable race state. Revisit orchestration only with explicit
failure evidence and a new decision; do not silently introduce target-error-driven rescue.

### TT-08: Preserve plausible race progression and results

Even when only selected situations are represented, any race progression and finish must follow
credibly from the actual course, field, selections, gaps, cooperation, fatigue and athlete actions.
Capabilities include sustained power, finite above-threshold work, sprint capability and physical
parameters; a “sprinter” or “climber” label is not itself a model or a winner rule.

Test the **path to contention**, not just the final ranking. For a sprinter winning a stage with
4,000+ m of elevation gain, inspect climb placement/gradient/duration, selection pace, regrouping
opportunities and reserve at the finish. Total ascent alone neither bans nor justifies that win.
An unexpected but explainable winner is valid; an unsupported route to the finish is not. Never
select a winner or alter capability to satisfy an expected archetype. Synthetic capability or
race-class assumptions must be explicit and must not be presented as empirical calibration.

### TT-09: Score independent dimensions

Report physiological workout fidelity, tactical recognition/timing, sequence continuity and race
plausibility separately. Overall placing is a consequence, not a substitute for any of them.
Neither winning nor producing target watts validates the lesson. A different wheel, a different
acceptable formation or an earlier catch is not automatically failure. Conversely, passing a
formation gate cannot excuse an unreadable move or a physiologically implausible result.

### TT-10: Keep observation distinct from authorship

Record planned situations separately from observed rider/group state, actions and outcomes. Keep
provenance for facts, model assumptions, estimates and inferred threat. An inferred mechanical
pace-demand estimate is not proof that every cheaper strategy fails. Post-finish cooldown is
judged for recovery, not an obligation to maintain a contested-race formation.

## Acceptance evidence

Before calling a situation validated, produce evidence for each dimension:

| Dimension | Minimum evidence |
|---|---|
| Recognition and fair information | Rider review with coaching/target hints hidden; intended facts available; reasons for responding or waiting inspected. |
| Context use and transfer | Matched scenes vary relevant team, strength, objective or course context; equivalent lessons also use unfamiliar names/colours. No permanent “follow this avatar” answer. |
| Effort and timing | Prescribed-power control plus a cue/action-following policy independent of target watts; measure effort, recovery, timing and local consequence. |
| Background threat | Ignore/cover comparisons in the stated context; show why ignoring does not require an emergency response, rather than requiring identical finishing places. |
| Continuity | Continuous execution through later steps with bounded noise, under/over effort and isolated timing errors; record lost opportunities without resets. |
| Progression and result | Inspect selections, gaps, effort/reserve history and route to contention, including contrasting course layouts and rider archetypes. |
| Technical integrity | Same kernel live/headless, fixed capability/terrain, deterministic replay, no synthetic measured telemetry. |

Compare recognizers/policies across explicitly described novice, experienced amateur and elite
assumptions when relevant. Proficiency is not an FTP multiplier: anticipation, cooperation,
positioning and power-duration profiles may differ independently. Quantitative distributions,
tolerances, scoring rubrics and sample sizes are still open. Software checks and a dogfooder ride
are useful evidence but do not establish perceptual training transfer on their own.

## Current implementation limits

`tools/race-prototype` supplies a synthetic all-zone fixture, frozen opponent commands and
provisional diagnostics ([ADR 0006](../decisions/0006-scripted-race-prototype.md)). Its nominal fit
and perturbation results do not establish TT-01 through TT-10. The original experiment does not
establish a validated recognition study, general situation compiler or empirical race-result
evaluator.
The live demo's spotlight tactics are an implemented policy, not evidence of lesson validity.

[ADR 0008](../decisions/0008-learning-contracts-and-public-reader.md) adds validated situation/public-
knowledge contracts, a public-motion reference reader, a target-independent cue-driven athlete,
separate diagnostics and practice observations to the tool. These implement useful seams, not
full validation: human recognition, public pull-duty communication, team cooperation, calibrated
future reachability and empirical result plausibility remain unresolved.
