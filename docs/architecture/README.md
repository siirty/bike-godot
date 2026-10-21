# Architecture

The product is a **Godot app over a Rust core**. Both run in one process, joined by a single
GDExtension. Decision records:
- [ADR 0001](../decisions/0001-repo-split.md): the repo split;
- [ADR 0002](../decisions/0002-rust-domain-port.md): the in-process Rust domain port;
- [ADR 0007](../decisions/0007-tactical-training-core.md): tactical learning situations and the
  director-first baseline, superseding mandatory live scene repair.

```
┌──────────────────────────────── bike-godot ─────────────────────────────────┐
│ apps/rider_godot          GDScript: screens, 3D road, camera, HUD, input    │
│        │  Godot classes: WorkoutLibrary, TrainerScan, TrainerLink,          │
│        │                 RideController, RaceController                     │
│        ▼                                                                    │
│ crates/godot_adapter      gd.rs (thin bindings) + app/ (plain Rust, tested) │
│        │                                                                    │
│        ├─ workout_import   ZWO/ERG/MRC → WorkoutPlan, versioned segment JSON│
│        ├─ session_director pre-ride terrain synthesis + training contracts  │
│        ├─ race_physics     forces, solvers, fixed-step dynamics             │
│        ├─ race_engine      deterministic kernel, PCG32, snapshots,          │
│        │                   RideSession (workout ride loop)                  │
│        ├─ trainer_control  TrainerControl, throttle, simulated trainer      │
│        ├─ local_storage    rusqlite workout library                         │
│        └─ ble_trainer      SIM-only TrainerControl ── trainer_core (git dep)│
└─────────────────────────────────────────────────────────────────────────────┘
```

Dependencies point downward only. The domain crates know nothing about Godot. Only
`godot_adapter` depends on `godot`, and only its `gd.rs` touches Godot types.

## Language split

- **GDScript**: presentation only, meaning layout, art, camera and input. It holds no domain
  rule, formatting rule or validation.
- **Rust**: everything else. `godot_adapter/src/app/` turns domain state into ready-to-show
  text and numbers, so even display decisions are unit-tested without Godot.

## Invariants (carried over from the prototype)

- **SIM mode only.** Trainers receive grade, wind, Crr and CdA. No path from Godot reaches an
  ERG/target-power write. `ble_trainer`'s command set has no such variant, and a test scans the
  crate's source for one.
- **Ride-loop code targets `TrainerControl`**, not BLE. The simulated trainer and the real one
  are swappable behind it. `ThrottledTrainerControl` is the only owner of the write-rate policy.
- **Explicit time.** Trainers, the throttle and `RideSession` take a monotonic `now` from
  Godot's frame loop. No timer or wall clock hides inside the core.
- **One deterministic kernel.** It uses fixed 50 ms steps, `pcg32-v1` randomness, canonical rider
  order and self-contained schema-v3 snapshots. Live and headless runs use the same kernel.
- **Capability lives in config, never in state.** Drafting scales aerodynamic drag only.
- **Measured telemetry is real.** Simulated outcomes never become recorded human power.
- **Regression evidence stays independent.** `fixtures/golden/` contains frozen historical vectors
  replayed by Rust tests. New contracts and their reference tests are owned here; no external
  code or toolchain generates expectations. See [ADR 0004](../decisions/0004-standalone-rust.md).

## Documents

- [`../requirements/tactical-training.md`](../requirements/tactical-training.md): normative core
  behavior TT-01–TT-10, including player context and result plausibility.
- [`physics-design.md`](physics-design.md): the accepted race, training and physics design. It covers the simulation contracts, positioning and drafting, the determinism guarantee, snapshots, dynamics and the timestep, situation planning, local AI policies, learning validation and fidelity. This is the source of truth for race behavior.
- [`open-race-design-questions.md`](open-race-design-questions.md): unresolved, implementation-gating questions by slice. It is not accepted architecture.
- [`build-order.md`](build-order.md): vertical slices, what is built, what is next, and how each slice is verified.
- [`product-alignment.md`](product-alignment.md): product identity, the first user and platform, the director-first approach, and the threshold-first vertical.
- [`trainer-core.md`](trainer-core.md): the `trainer_core` boundary, the SIM-only chokepoint, and the write-rate policy.
- [`risks.md`](risks.md): platform and implementation risks to budget for.
- [`platform-rollout.md`](platform-rollout.md): Linux first, then Windows, Android, and macOS/iOS.
- [`../trainers/`](../trainers/): per-trainer hardware datasheets. These are records, not requirements.
- [`../port/divergences.md`](../port/divergences.md): deliberate differences from the Dart prototype.
- [`../research/race-calibration-data-spike.md`](../research/race-calibration-data-spike.md): a time-boxed spike on sourcing real race data for calibration.

When a decision is accepted, update its authoritative document and remove the matching open question in the same change. Edit stale text in place; do not append correction notes.

The [focused selection-chain experiment](../decisions/0009-cue-calibrated-selection-chain.md)
separates motion execution from public-cue interpretation and rejects unsuitable pre-ride fits.
Its [verification record](../verification/2026-10-21-selection-chain.md) distinguishes bounded
engineering acceptance from human learning and empirical race plausibility.
