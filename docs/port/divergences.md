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
