# ADR-0002: Port the Dart domain core to Rust, in process

- Status: accepted
- Date: 2026-10-20
- Owners: Levin Schmidt (siirty)
- Related: supersedes point 4 of [ADR 0001](0001-repo-split.md) and its "two runtimes" consequence

## Context

ADR 0001 kept the Dart domain core (`workout_import`, `race_physics`, `session_director`,
`race_engine`) in `bike-training-app`. It was to run as a headless process behind a replay/IPC
contract, and a port was deferred until golden vectors existed. Under that plan the Godot app
would ship two runtimes, and every domain call would cross a process boundary. The owner then
decided to move fully to Godot + Rust.

The precondition ADR 0001 set for a port, golden vectors recorded from the Dart reference, is now
met. `tools/golden_exporter/` records them into `fixtures/golden/`.

## Decision

1. Every Dart package is ported to a Rust crate of the same name under `crates/`:
   - `workout_import`
   - `race_physics`
   - `session_director`
   - `race_engine`
   - `trainer_control`
   - `local_storage`
   - `ble_trainer`

   `ble_trainer` sits directly on `trainer_core`; no FFI is needed.
2. The domain crates are parity-tested **bit for bit** against vectors recorded from the Dart
   prototype. Doubles round-trip exactly, and race snapshots are byte-identical. Every intentional
   difference is listed in [`docs/port/divergences.md`](../port/divergences.md) and named in the
   test that would otherwise catch it.
3. The Godot app (`apps/rider_godot`) calls the crates **in process** through one GDExtension,
   `crates/godot_adapter`. That gives one runtime, with no IPC protocol and no Dart toolchain.
4. GDScript stays presentation only: layout, camera, art and input. Formatting, validation,
   persistence, physics and trainer policy live in Rust, so they are unit-tested without Godot.
5. The Flutter app stays the frozen reference and oracle, as ADR 0001 says. The golden exporter
   is the only thing that still runs Dart.

## Alternatives considered

### Keep Dart as a sidecar process (ADR 0001)

Rejected by the owner's decision. It ships two runtimes, needs a live IPC protocol before the
first ride, and splits debugging across a process boundary.

### Port in one shot, without vectors

Rejected. The kernel's value is determinism; without an oracle, a port silently changes how
saved races resume.

## Consequences

- The invariants carry over unchanged and are enforced in Rust:
  - SIM mode only;
  - one deterministic fixed-step kernel;
  - `pcg32-v1`;
  - capability in config, never in state;
  - drafting changes aero drag only;
  - self-contained schema-v2 snapshots.

  `ble_trainer` exposes no ERG path, and a test scans its source for one.
- Time is explicit. `TrainerControl` and `RideSession` take a monotonic `now` from the engine's
  frame loop, replacing Dart's timers and streams.
- Dart's codegen (freezed, json_serializable, drift) is replaced by serde and hand-written
  rusqlite. The schema is pinned by tests instead.
- A change to the Dart prototype no longer reaches the product. The prototype is frozen, so a
  new rule is written in Rust with its own tests. Regenerate the vectors only to re-check parity.

## Verification and review

`scripts/project-ci.sh` runs:

- `cargo fmt`, `cargo clippy -D warnings` and `cargo test` across the workspace, including
  every golden suite;
- a build of the extension;
- a headless Godot smoke test that imports a workout, rides it on the simulated trainer, and
  steps the live race kernel.

Revisit this decision if a target platform cannot load a GDExtension. Godot Web would be the
first candidate for that.
