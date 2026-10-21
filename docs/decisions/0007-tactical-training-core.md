# ADR 0007: Tactical learning situations define the trainer's core

- Status: accepted product direction and initial orchestration baseline; implementation pending
- Date: 2026-10-21
- Supersedes: equal-priority full-race framing and mandatory live scene-repair architecture in the
  2026-09-21 product alignment / physics design; future-planner expectations in ADR 0005 only.
- Retains: ADRs 0001–0005 implemented boundaries, SIM-only control, one deterministic kernel,
  fixed capability, actual-power authority and honest measured telemetry. ADR 0006 remains an
  experimental tool record, not a production-readiness claim.

## Context

The user clarified that the goal is to train the tactical eye through workout-driven race
situations, not replicate complete races. Players need enough information about riders,
affiliations, strengths, role and terrain to judge moves. Background movement must be legibly
safe to ignore. Local lessons must still yield credible race progression and results: neither
workout fidelity nor a desired rider archetype excuses an implausible finish.

Earlier documents required a live Scene Planner to maintain training-equivalent continuations.
The current direction instead starts with deterministic director planning together with terrain,
without automatically repairing ordinary rider errors. The all-zone prototype explored frozen
opponent commands, but its geometric gates are not validated tactical or workout criteria.

## Decision

1. The tactical learning situation is the unit of authoring and validation. The workout defines
   effort; recognizable context and actions give the player a reason and a time to produce it.
2. Specify player knowledge and local objective alongside cues, acceptable responses, effort
   envelopes, consequences and continuity. A small roster/briefing and recognizable affiliations
   are required when the lesson depends on them; a full simulated race world is not.
3. Treat safe-to-ignore background moves as contextual claims to test. Following may improve
   position without making a move mandatory. Never reveal the classification as the essential cue.
4. Start with a deterministic pre-ride sequence, frozen terrain/capability and bounded opponent
   intentions/roles. Local reactive tactics remain allowed. Do not require a general live Scene
   Planner or silently add error-repair, target tracking, teleportation or capability changes.
   Exact frozen command frames are an experimental option, not mandated for all production AI.
5. Validate workout fidelity, tactical recognition/timing, continuity and race plausibility
   independently. Inspect the path to a result from terrain, capabilities, fatigue and selections;
   do not prescribe a winner. Total elevation alone is not a result-validity rule.
6. Treat ordinary athlete errors as real consequences. Validate future opportunity availability
   within explicit reference bounds before riding; do not promise viability for arbitrary input.
   Technical-disconnect pause semantics remain separate from gameplay failure.

Normative requirements and acceptance evidence live in
[TT-01–TT-10](../requirements/tactical-training.md). Ownership and current implementation limits
live in [physics-design.md](../architecture/physics-design.md).

## Consequences and remaining decisions

A general autonomous whole-race benchmark or exact formation match cannot be the sole acceptance
gate. Recognition tests need human observation and context-transfer cases; action-following tests
must not receive prescribed watts. A surprising finish may be valid; a scripted lesson is invalid
if it depends on missing player information or physically unsupported selections.

General runtime replanning is no longer a required numbered slice. Basic affiliations/information
are brought forward where required; richer team simulation and persistent career systems remain
later scope. Existing code, schemas, fixtures and prototype results are unchanged by this ADR.
Situation/knowledge/result-evaluator schemas, numerical tolerances, scheduling anchors and
proficiency calibration remain open. If the initial approach cannot meet the requirements, record
specific evidence and decide any replacement or bounded adaptation explicitly.
