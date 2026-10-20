# Deliberate divergences from the Dart prototype

The Rust crates are parity-tested against vectors recorded from the Dart prototype
(`fixtures/golden/`, see `tools/golden_exporter/`). Where the port intentionally behaves
differently, the difference is listed here **and** special-cased by name in the relevant
`tests/golden.rs`, so no divergence is silent.

## workout_import

| Case | Prototype | Port | Why |
| --- | --- | --- | --- |
| `.zwo` duration is negative, NaN or infinite | Builds a negative `Duration`, or throws an unrelated `UnsupportedError` | Rejected with a format error naming the attribute | A negative segment breaks every timeline invariant. `std::time::Duration` cannot be negative. |
| Stored envelope has an integer `version` but `segments` is not a list | Reports `version: null` | Reports the version it read | The prototype's own doc comment says the version is reported "if one could be read at all". |
| `WorkoutPlan.name` | `String?` | `Option<String>` | Same semantics. Listed only because it's a type-level choice. |

The interval-repeat invariant (an off phase needs both off powers) is enforced on every
construction path in the port. In the prototype it is a freezed `assert`, so it only applies
when asserts are on (as in its test suite). The vectors are recorded with `--enable-asserts`,
so the two sides agree.

## trainer_control

| Case | Prototype | Port | Why |
| --- | --- | --- | --- |
| Time and events | Broadcast `Stream`s, `Timer.periodic`, injected `DateTime` clock | Caller-supplied monotonic `now: Duration` and a pull-based `poll(now)` | An engine frame loop drives the trainer, and tests use literal times instead of `fake_async`. No wall clock hides inside a trainer. |
| Throttle flush | A `Timer` fires exactly at `lastWrittenAt + interval` | Flushed on the first `poll` at or after that instant, stamped with the poll's `now` | Same policy. The flush lands at most one frame later. |
| Simulated trainer given more power than 30 m/s needs | The solver throws inside a timer callback (uncaught) | Holds 30 m/s | A dev slider must not crash the ride. |
| Simulated trainer given negative or non-finite power | The solver throws on the next tick | Treated as 0 W | Same reason. |
| `RecordingManualPowerTrainer` | A subclass | `RecordingTrainerControl::with_manual_power()` | Rust has no subclassing. The capability is still opt-in. |

## session_director

| Case | Prototype | Port | Why |
| --- | --- | --- | --- |
| `_DeterministicLcg` | Constructed from the seed and never drawn from | Not ported. The seed parameter is kept and documented as currently unused | Dead code. Output is identical for every seed on both sides. |
| `SegmentDistanceRange.segment` | `Object` reference | The segment plus its plan index | Typed, and it survives a skipped zero-duration segment. |
| A plan whose segments are all zero-duration | `StateError` from `samples.last` | `SynthesisError::NoDistance` | Typed error instead of a crash. |

## race_engine

The original Dart force, solver, dynamics, RNG and terrain vectors remain unchanged. Historical
race vectors are replayed using test-only v3 inputs with null physiology and a zero drafting
minimum gap. Their state and projected legacy snapshot bytes remain exact. Production v3
snapshots require the new fields and reject v2; they have independent round-trip and resume tests.

| Case | Prototype | Port | Why |
| --- | --- | --- | --- |
| Physiology | Requested power is unlimited | Optional fixed CP/W′/Pmax and remaining reserve bound delivered power | Software riders have finite capability; reference fixtures explicitly omit it. |
| Snapshot schema | v2 | v3, with physiology, reserve and minimum gap; v2 rejected | Changed continuation semantics must be explicit. |
| Drafting overlap | Shelter extends to any positive gap | Default 2.5 m center-to-center minimum in all eligible slots | Removes shelter between overlapping riders. |
| Decoded `maxPropulsiveForceN` ≤ 0 | An untyped `ArgumentError` escapes the codec | `validation:notPositive` at `config.dynamics.maxPropulsiveForceN` | The codec promises typed errors with a path. |
| A physics failure inside a step (drivetrain efficiency above 1, a non-finite result) | An untyped `ArgumentError`/`StateError` escapes `step` | A typed validation error on `state.riders[i].<field>` | Same promise, for the kernel. |
| `"not valid JSON: …"` detail | `dart:convert`'s wording | `serde_json`'s wording | Parser-specific text. Code, path and prefix match. |
| `RideSession` time | `Timer.periodic` + telemetry stream | `advance(now)` replays each due render tick, polling the trainer at the tick's own time | Engine-driven frame loop. Slicing time finely or coarsely yields identical frames (tested). |
| `RideSession` clock after a telemetry snap | Snaps to the trainer's elapsed time, *then* adds a render interval, so the frame runs one interval ahead | Adds the interval first, then snaps | A bug fix. Each frame's `elapsed` is now its own tick time on a trainer whose clock agrees. |
| `RideSession` grade-write failure | Unawaited future (unhandled async error) | Returned in `RideAdvance::write_errors`; frames continue | Nothing is silently lost. |
| `RideSession.reportStartFailure` | Pushes a start error onto the frame stream | Not ported | `start` returns a `Result` to a caller that can always see it. |
| `RideFrame.currentSegment` | Object identity with the plan's segment | A clone plus `segment_index` | Rust has no shared identity to compare. |

## local_storage

There are no golden vectors: the Dart package is a column mapping over drift. The Rust tests port
its repository and migration suites and check the same behaviour against both implementations.

| Case | Prototype | Port | Why |
| --- | --- | --- | --- |
| Engine | drift (`database.g.dart`) | rusqlite with hand-written SQL; schema version in `PRAGMA user_version` | No codegen. The test pins the exact column list, nullability and primary key. |
| Timestamps | drift `storeDateTimeAsText` (local ISO-8601) | Fixed-width UTC `YYYY-MM-DDTHH:MM:SS.ffffffZ` | Text order is time order, in any timezone. |
| Same-instant saves | Order undefined | Tie broken by insertion order (`rowid`) | "Newest first" holds without a sleep in the test. |
| `watchSavedWorkouts()` stream | Reactive drift query | `revision()` counter; callers re-list | A frame-driven UI polls; no async runtime in the store. |
| Clock | `DateTime.now()` inline | Injectable `Clock` (default: system clock) | Deterministic ordering tests. |
| Upgrade from an unknown schema version | `StateError` from `onUpgrade` | `LocalStorageError::NoMigration`, and nothing is created in the file | Same policy, typed. |
| `SavedWorkoutRecord.plan` / `unreadable` | Two nullable fields plus an assert | `Result<WorkoutPlan, UnreadableWorkout>` | "Never both, never neither" is the type. |
| `saveWorkout(sourceFormat: String)` | Any string | `WorkoutFormat`; reads keep the stored text | Only real parser tags are written. |

## ble_trainer

The Dart package was an FFI adapter over `trainer_core`. The Rust crate calls `trainer_core`
directly, so there are no golden vectors. Its tests drive the command worker against a fake
`TrainerConnection` and check the SIM-only surface.

| Case | Prototype | Port | Why |
| --- | --- | --- | --- |
| Transport | flutter_rust_bridge bindings + Dart streams | A tokio worker task per trainer. Commands go over a channel; `poll(now)` drains telemetry and state | One process, no codegen. The engine's frame loop pulls events. |
| ERG reachability | No ERG binding generated | The worker's command enum has no target-power variant, and a test greps `src/` for ERG symbols | Same rule, checked mechanically. |
| Core-simulated trainer | Rust sleeps 500 ms between reports | `CoreSimulatedTrainer` steps `trainer_core`'s simulator on caller time: 100 ms ticks, 500 ms reports | Deterministic tests with no sleeps. |
| Async failures | Errors surface on a stream | Collected and returned by `take_failures()`, then shown in the ride's message line | Nothing is silently lost. |
| Connecting from a scan | Scan and connect were independent | `TrainerScanner::connect` stops the scan first | Some BLE stacks refuse to connect while scanning (BlueZ in particular). |

## Godot app (`apps/rider_godot`, `crates/godot_adapter`)

| Case | Prototype (Flutter) | Port | Why |
| --- | --- | --- | --- |
| FTP | Fixed in the session | Set in Settings, clamped to 100–400 W in the UI and 50–600 W in the extension, and saved in `user://settings.cfg` | A ride needs a rider; this is the smallest durable setting. |
| SIM debug screen | A full screen with controls | A toggleable panel showing the simulated trainer's state | Power is already set by the ride screen's slider. |
| Race view | Replays a JSON file exported by a Dart tool | Steps the Rust kernel live at a fixed 50 ms, using the exporter's scripted powers | No Dart toolchain. It is the same kernel the vectors check. |
| Ride clock | — | Snaps to FTMS elapsed time, which has whole-second resolution. With 500 ms reports the HUD clock can move by up to 0.5 s | Inherited from the prototype's `RideSession`; recorded so it is not mistaken for a port bug. |
| Display formatting | Dart widgets format values inline | `godot_adapter::app::format` produces the text, and GDScript only places it | Unit-tested without Godot. |
