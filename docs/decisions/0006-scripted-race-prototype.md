# ADR 0006: All-zone experiment with a frozen race-action schedule

- Status: accepted for an experimental tool; production orchestration remains unimplemented
- Date: 2026-10-21
- Follow-up: [ADR 0007](0007-tactical-training-core.md) defines the situation-training product
  direction and director-first baseline; this tool remains experimental and its gates provisional.

## Purpose

Test the user's alternative to a general runtime Scene Planner: author workout-driven opponent
intentions and terrain before the ride, then execute them without correcting for the athlete.
This experiment deliberately covers Z1–Z7 plus recoveries. It is an engineering stress fixture,
not a training prescription, universal zone model or a completed TrainingContract/evaluator.

## Boundary

`tools/race-prototype` drives the existing 50 ms production kernel. It neither implements new
physics nor changes simulation contracts, snapshots, Godot or measured activity storage. The
"athlete" is a software reference rider, never hardware telemetry.

A deterministic offline compilation pass uses a nominal prescribed-power rider to calibrate
opponent power/braking/slot intentions against phase-specific target geometry. Calibration may
observe this offline reference state; it cannot rewrite positions or capabilities. Recorded
intentions are frozen before testing variants. Runtime replay has no reference-state feedback,
role replanning, rubber-banding, reset between phases or catch-up recovery.

Terrain is generated once as distance-based grade samples and held identical across variants.
Every replay begins at the same state. Fixed CP/W′/Pmax limit actual power in every pass.
The intended sprint finish distance is frozen from the offline reference pass.

## Validation

Separate schedule transitions from observed geometry. Report phase entry/exit gaps, acquisition
latency, observed wheel availability, delivered work, reserve, power shortfall and an independent
mechanical demand estimate at the pacer's actual speed/acceleration and observed shelter.
Compare that estimate with the prescribed effort; replaying prescribed watts alone proves nothing
about induced demand. Also run a simple named-wheel follower, small under/over and late inputs,
ignore/cover-decoy pairs, and one deliberately missed hard effort. Opponent commands must be
identical across every variant. Failed transitions remain failures, not repaired states.

Tactical significance is an authored hypothesis. The report includes observable capability,
geometry, and ignore-versus-cover consequences; it does not claim to prove natural tactics or
race threat from a label. Human observation, calibrated profiles and real hardware remain needed.
