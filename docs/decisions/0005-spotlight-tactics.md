# ADR 0005: Explicit-time utility tactics for spotlight riders

- Status: accepted for implemented tactics; future mandatory Scene Planner expectations superseded by ADR 0007
- Date: 2026-10-20
- Scope: next Slice 5 component, issue #1

## Decision

Add `race_engine::tactics` as an input policy over the existing kernel. Only two or three
spotlight riders use it; the rest retain seeded pack pacing. Controllers read observable race
positions/speeds, their fixed personality and their own physiology. They never receive workout
structure, target error, fidelity or a SceneGoal. They request only power, slot and wheel intentions.
Capability limiting, drafting, movement and race events remain in the one kernel.

Evaluate recover/cruise/follow/chase/attack/sprint utilities at an explicit configurable cadence
(default 1 s), with a minimum commitment (3 s) and a switch margin (0.10). Low reserve interrupts
commitment; recovery has separate enter/exit thresholds (20%/40%) to avoid chatter. No wall clock
or library RNG is used. Seeded personalities are generated once with PCG32. Ties have a fixed
ordered-action priority and canonical rider-ID target selection. Targets are local (100 m default);
finished riders are excluded. A sprint is allowed only on a course with an explicit finish.

The minimal planner seam is `TacticalRole::{Neutral, Instigator, Chaser, Conserving}`. A role adds
at most 0.15 utility to a valid action; it never enables an invalid action, changes capability or
bypasses mandatory recovery. Roles are inputs held until the next decision, not goals injected
into local AI. This is the bounded influence boundary only. [ADR 0007](0007-tactical-training-core.md)
supersedes the expectation of a required later live Scene Planner; preplanned roles can use the
same boundary without runtime repair.

Controllers hold their last action, target, power request, decision time and commitment start.
Kernel snapshots still cover kernel continuation only. To resume an autonomous race exactly,
store the controller checkpoint alongside the kernel snapshot and reapply identical future roles
and pack plans. Checkpoints have their own version and validate configuration, personality,
identity, times and targets before adoption. This avoids hiding controller state in the kernel.

All numbers are provisional configurable fixtures. Following uses a proportional gap/speed power
request capped by physiology; it is not collision avoidance. Persistent reputation, calibrated
profiles, tactical cooperation and scene viability remain open. The live demo's human stays
scripted until Slice 9; opponents use this controller. A headless mixed-field corpus exercises the
same policy. No human/trainer integration is claimed.

## Acceptance and verification

Specify reaction tests (close wheel, break ahead, exhaustion, finish), cadence/commitment,
bounded roles, invalid inputs and exact controller-plus-kernel continuation. Run fixed seeds
through the existing harness; configuration remains unchanged and all riders finish. Historical
regression vectors remain unchanged. Report observed action counts without claiming naturalness.
