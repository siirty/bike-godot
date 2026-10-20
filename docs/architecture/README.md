# Architecture

The product is a **Godot app over a Rust core**. Both run in one process, joined by a single
GDExtension. Decision records:
- [ADR 0001](../decisions/0001-repo-split.md): the repo split;
- [ADR 0002](../decisions/0002-rust-domain-port.md): the in-process Rust domain port.

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
  order and self-contained schema-v2 snapshots. Live and headless runs use the same kernel.
- **Capability lives in config, never in state.** Drafting scales aerodynamic drag only.
- **Measured telemetry is real.** Simulated outcomes never become recorded human power.
- **Parity is proven, not assumed.** `fixtures/golden/` holds vectors recorded from the Dart
  prototype, and each crate's `tests/golden.rs` replays them bit for bit. Deliberate differences
  are listed in [`docs/port/divergences.md`](../port/divergences.md).

## Not yet built

- A workout-driven race. `session_director`'s scenarios do not feed the live kernel yet, and the
  race screen runs a scripted three-rider scenario.
- AI tactics.
- Activity recording.
- Sync.
- Packaging.

Native BLE has only been exercised on Linux. Do not claim support on any other platform until it
has been proven on real hardware.
