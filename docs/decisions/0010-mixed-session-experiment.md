# ADR 0010: Playable mixed-session software-rider experiment

- Status: accepted for this scoped engineering experiment
- Date: 2026-10-22
- Extends ADR 0006–0009 and TT-01–TT-10; no general lesson compiler is claimed

## Decision

Add a separate mixed-session screen backed by the same pure 50 ms RaceSimulation kernel
as headless execution. It accepts explicit software power, slot and wheel intentions.
It has no trainer connection, activity writer or ERG operation. Real trainers retain the
existing SIM-only ride path. Trainer-authoritative race reconciliation remains a later slice.

The immutable engineering workout has eight contiguous blocks: 600 s ramp 45–68% FTP,
900 s at 68%, 180 s at 110%, 600 s at 68%, 360 s at 82%, 60 s at 120%,
600 s at 68% and 300 s ramp 55–35%. These are fixture settings, not an individual
prescription. The session ends after 3,600 s; there is no fabricated finish distance/winner.

Six persistent riders retain fixed mass, aero, CP, W′ and peak power across every block.
CP equals FTP for the software reference only as an explicit simplifying assumption.
W′ is 20 kJ. NPC peak and CP ratios vary by roster entry; no capability changes at runtime.
Terrain is frozen by distance. Public affiliation and qualitative capability use the existing
PlayerKnowledge contract, separately from private reserve/requests and authored annotations.

Following ADR 0006's experimental boundary, fit one supporting rider's ordinary power/braking
intentions offline around nominal prescribed input, then freeze that script before variants.
No positions or speeds are imposed. The other pack riders use local following, finite-reserve
recovery and bounded attack opportunities. The spotlight rider uses the existing TacticalController,
with a fixed cruise/recovery tuning of 60%/55% own CP and preplanned bounded roles. Generic
90% CP cruising escaped the warm-up field in the first comparison; this explicit fixed policy
change retains a useful local field without modifying capability or adding live target feedback.

Opponent controllers never receive athlete target error/fidelity or the hidden lesson answer.
The block schedule selects bounded roles (Conserving, Chaser, Neutral, Instigator), not a live
rescue policy. Missing effort or losing shelter can leave the athlete behind for subsequent
blocks. A diagnostic fallback instruction acknowledges independent work; it does not rejoin riders.

## Observation and presentation

A reusable RaceObserver projects actual riders into longitudinal groups. Defaults are a 12 m
connection threshold, 3 s establishment persistence, 5 s trend window and 0.15 m/s deadband.
These are configurable, provisional engineering values. Membership changes reset trend history;
a reduction in group count reports regrouping. Numeric gap trend remains distinct from explicit
policy intent: stable geometry never proves deliberate management. Only the front/next-group
relationship is classified, not a full cooperation/threat model or the complete race-state graph.
Observer continuation stays outside existing kernel events and snapshot schemas.

Essential objective, roster and actual rider labels remain visible. Coaching, targets, state
annotations, reserve and NPC requested power are hidden by default with an explicit reveal.
The headless viewer is an engineering inspector that intentionally exposes those diagnostics.
None of these displays is a validated recognition assessment or tactical scoring system.

## Replay and limits

Godot exports a versioned software-only audit with the immutable initial kernel snapshot,
timestamped control changes, end time and one-second diagnostic samples. The loader validates
policy version, initial snapshot, duration, timing, identities, finite power and slots before
resetting. Playback executes recorded ordinary controls through MixedSession, preserving all
controller continuation by replaying from the start. It stops at the recorded end time and
locks input changes until restart. This is not a cross-version replay guarantee, FIT export or
a new kernel snapshot format. No simulated sample enters measured activity storage.

Nominal target adherence alone cannot validate naturally induced effort: pace was fitted to
that reference. Independent cue-policy execution, bounded execution acceptance, tactical/result
plausibility and human transfer remain open. Public team labels provide necessary context, but
do not imply general team tactics, GC, empirical calibration or collision avoidance.
