# ADR 0008: Validated learning contracts and a public-cue reference reader

- Status: accepted for the first executable learning vertical; heuristic calibration pending
- Date: 2026-10-21
- Implements parts of ADR 0007 and TT-01–TT-10; does not claim all acceptance evidence exists

## Decision

`session_director::situations` owns a version-1, pre-ride `LearningPlan`, validated situation
specifications and separate `PlayerKnowledge`. Contracts reject missing/duplicate identities,
invalid effort/timing, noncontiguous schedules and missing author explanations. Serialization is
export-only; no unchecked deserializer or persistence/resume format is introduced. General
workout recognition and a production scenario compiler remain future work.

Public knowledge contains identity, display name, optional affiliation, qualitative reputation,
objective and briefing. It contains no exact opponent physiology, authored answer or schedule.
`race_engine::perception` converts the existing kernel state to observable motion and current
course context. Its reference reader receives only that projection, public knowledge and explicit
settings. It can distinguish shelter, monitor, teammate-covered movement, responding, sprinting
and a crossed finish. These are engineering heuristics, not trained human classifications.

The all-zone tool authors one contract per fixture phase, exports the roster/context and runs an
additional cue-driven software athlete with no workout/phase input. The old phase-informed wheel
follower remains an explicitly labelled comparison. The kernel, frozen opponent commands and
physical capabilities remain unchanged. New dependency edges use existing workspace serde and
session_director; no new third-party package or cyclic domain dependency is introduced.

## Diagnostics and evidence boundaries

The tool records at 20 Hz:

- actual effort share in the original authored envelope, independent of legacy geometry gates;
- a provisional contention proxy accepting alternative wheels and riding ahead;
- observed reference-reader cue timing and the next >5 W requested-power rise;
- delivered work, peak per-step power, minimum reserve and largest climbing deficit for each rider;
- actual kernel finish times/places, without an expected-winner rule.

Defaults (8-second window, 60 m contention bound, 80% time in ±10% FTP) are explicit diagnostic
assumptions, not calibrated contracts or proof of future effort reachability. A power rise is not
proof of intent; no human recognition score is inferred. Post-race/scheduled cooldown has no race
formation gate. Mechanical state validation does not prove empirical race plausibility, which
remains `NeedsReview` with trajectory evidence available. No archetype or total-ascent veto is used.

Playback defaults to context-and-motion practice with coaching/diagnostics hidden. The athlete's
own telemetry, roster and affiliation remain available. Participants can record a response and
reasoning, then explicitly reveal the reference comparison. Exported observations are unscored
practice data, not measured activities. Finish events are displayed only after their observed time.
The diagnostic view can intentionally reveal full-run evidence; it is not a blinded assessment.

## Limits and follow-up

The fixture has no public pull-duty signal, and reports that limitation rather than marking tempo
recognition validated. Opponent team cooperation is not yet modeled; affiliations affect the
reference interpretation, not the frozen opponent script. The reader is local and deliberately
simple, with no claim that its motor policy produces every workout envelope. Failures remain
visible. Human cue/transfer studies, course/proficiency calibration, production scheduling,
reachability checks, full path-to-result validation and live Godot/trainer integration remain open.
