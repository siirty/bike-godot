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
