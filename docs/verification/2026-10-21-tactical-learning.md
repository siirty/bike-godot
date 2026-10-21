# Tactical learning vertical verification — 21 October 2026

## Scope

Implements validated pre-ride learning/public-knowledge contracts, sanitized observable motion,
a public-cue reference reader and an additional workout-independent software athlete. The
all-zone playback adds roster/context, practice observations and separate diagnostic dimensions.
See [ADR 0008](../decisions/0008-learning-contracts-and-public-reader.md).

No changes to physical formulas, kernel stepping, snapshot codecs, existing fixtures or trainer
control were made. New serde/session_director dependency edges use existing pinned packages.
The report is version 2; it is not a kernel snapshot or measured activity format.

## Executed checks

- 138 Rust tests passed across race_engine, race_physics, session_director, workout_import,
  trainer_control and race_prototype, including 15 added tests. These cover contextual affiliation/
  objective changes, uncertain knowledge, canonical public names, invalid contracts, independent
  cue execution under changed workout labels/targets, alternative formations, cooldown exemption,
  honest review-pending dimensions, inconsistent plan rejection and actual work/finish witnesses.
- Clippy over those six packages, all targets, with warnings denied passed.
- Root `cargo fmt --all --check` passed.
- `python3 scripts/check-fixtures.py` verified four unchanged frozen fixture files.
- Browser checks: eight replay policies, context retained with coaching hidden, no visible
  reference answer before response, explicit comparison, reasoning export, separate audit tables,
  phase navigation, scrubbing, playback and mobile width. No JavaScript errors; no horizontal
  page overflow at 390 px. Desktop/mobile screenshots were inspected.
- Relative documentation links and `git diff --check` passed.

The Rust checks used an isolated copy of these six packages and the repository's pinned lockfile,
with only unused workspace members excluded: the private trainer dependency was unavailable in
this execution environment. Godot, BLE, storage and physical trainer integration were not tested.
With normal repository dependency access, reproduce the domain checks using:

```sh
cargo test -p race_engine -p race_physics -p session_director -p workout_import -p trainer_control -p race_prototype --release
cargo clippy -p race_engine -p race_physics -p session_director -p workout_import -p trainer_control -p race_prototype --all-targets -- -D warnings
cargo run -p race_prototype --release -- --out race-prototype-output
```

## Observed default fixture, 250 W / 85 kg / seed 42

The original seven controls retain their previous trajectories and finish outcomes. The new
cue-driven policy has no phase or prescribed-watt input. Its threshold effort is within the
original ±10% FTP band for approximately 83% of the block, but it misses several other envelopes
and finishes at 935.7 s, before the authored sprint block. This is visible failure evidence, not
an accepted all-zone training result or proof that the reader is realistic. The old formation
gate count is retained as a legacy diagnostic, not a lesson score.

Human recognition and empirical result plausibility remain `NeedsReview`. Mechanical validation,
work/reserve history, climbing deficits and actual finish events support review but do not
establish tactical validity. Tempo pull-duty communication, opponent team cooperation, general
workout-to-situation compilation and future effort reachability remain implementation gaps.
