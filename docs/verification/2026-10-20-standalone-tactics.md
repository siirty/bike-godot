# Standalone Rust and spotlight tactics verification — 20 October 2026

Base: Rust-port branch at `f8089346b003b814c4cb169ba84d4faa1a7960bb`. Implements #1.

## Environment

Linux x86_64, Rust 1.99.1, official Godot 4.5.1 and 4.7.2. No Dart/Flutter toolchain or legacy
checkout was used. CLI access to private `trainer_core` was unavailable, so its exact pinned
`49bde0bd25bed2ce01eb1b1ceb9bb857e5240089` source files were fetched through the GitHub connector and
used through a temporary local Cargo patch. No patch or lockfile change is committed. D-Bus
build headers and pkg-config were unpacked locally; real Bluetooth was not exercised.

## Results

- `python3 scripts/check-fixtures.py`: all four frozen regression vectors unchanged.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 151 tests passed, zero failed (including nine tactical tests).
- Final adapter-only tests: 12 passed after removing obsolete opponent power scripts.
- `cargo test -p race_engine --test tactics --release`: nine passed.
- `cargo build -p godot_adapter`: passed.
- Godot headless smoke scene: zero failures on both engine versions, including import, simulated
  ride, race completion and screen construction. These are runtime smoke checks, not visual QA.
- **Full `scripts/project-ci.sh` is not green on a fresh import.** Rust stages and the extension
  build pass, but the first Godot editor import aborts. Subsequent import and smoke checks pass.
  The same cold-import failure was reproduced after separately building the untouched base
  commit, and a minimal empty project imports successfully. Tracked as #2; no retry or skip was
  added to CI. Disabling reloadability in a throwaway copy did not fix it.

## Seed corpora

Both commands use the existing production stepping kernel. They have no legacy-toolchain step:

```sh
cargo run -p race_engine --features testing --example headless_corpus --release -- 0 16
cargo run -p race_engine --features testing --example tactical_corpus --release -- 16
```

The baseline finished every rider in all 16 seeds; mean winner speed 40.4 km/h, mean field spread
155.0 s. The mixed field also finished every rider in every seed. The table below counts
spotlight rider-steps spent in each action (two spotlight riders, 50 ms steps), not decisions or
calibrated naturalness. All six actions are exercised across the corpus.

| seed | finished | steps | recover | follow | chase | cruise | attack | sprint |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 9 | 39275 | 1000 | 27520 | 11755 | 36375 | 1000 | 900 |
| 1 | 7 | 17808 | 2680 | 11700 | 8383 | 12388 | 0 | 465 |
| 2 | 9 | 16390 | 2160 | 5160 | 8523 | 16417 | 0 | 520 |
| 3 | 6 | 18373 | 1540 | 3200 | 5894 | 23753 | 1860 | 499 |
| 4 | 9 | 17923 | 0 | 31440 | 2440 | 1303 | 163 | 500 |
| 5 | 9 | 17493 | 3921 | 16960 | 12524 | 1160 | 0 | 421 |
| 6 | 9 | 18361 | 0 | 31580 | 3860 | 662 | 60 | 560 |
| 7 | 10 | 14251 | 1180 | 11340 | 2793 | 10769 | 1900 | 520 |
| 8 | 6 | 21929 | 2340 | 24240 | 7933 | 8709 | 0 | 636 |
| 9 | 10 | 17499 | 1160 | 23900 | 3074 | 4584 | 1780 | 500 |
| 10 | 10 | 18539 | 2480 | 17660 | 6462 | 5836 | 4040 | 600 |
| 11 | 7 | 22372 | 278 | 28960 | 11460 | 3272 | 0 | 774 |
| 12 | 7 | 18342 | 7440 | 14100 | 12732 | 2335 | 0 | 77 |
| 13 | 10 | 17393 | 1060 | 14300 | 6647 | 10199 | 2260 | 320 |
| 14 | 9 | 12541 | 0 | 13200 | 2380 | 7422 | 1640 | 440 |
| 15 | 9 | 15738 | 3220 | 10820 | 9986 | 7015 | 0 | 435 |

## Remaining scope

Empirical rider/profile calibration, richer threat and cooperation behavior, collision avoidance,
the full Scene Planner, workout contracts/evaluator, human race inputs and physical trainer
verification remain later work. A kernel snapshot alone still does not contain an input policy;
autonomous continuation additionally stores each controller checkpoint and identical future
roles/pack plans. Snapshot schema v3 and persisted workout encodings are unchanged.
