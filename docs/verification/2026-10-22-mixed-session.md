# Mixed-session rebuild verification — 22 October 2026

Base: `c58e5d28df2456b78f820cc42cf949039954d16d`, the cue-selection branch.
The implementation was rebuilt and all results below were rerun; they do not rely on
the unavailable previous implementation. [Usage](../architecture/mixed-session-experiment.md)
and [ADR 0010](../decisions/0010-mixed-session-experiment.md) describe the scope.

## Environment and dependency handling

Linux x86_64, Rust 1.94.0, official Godot 4.3, Mesa llvmpipe/OpenGL Compatibility and Xvfb.
CLI access to private `trainer_core` returned 401. Its exact pinned
`49bde0bd25bed2ce01eb1b1ceb9bb857e5240089` source was fetched through the GitHub
connector and all 23 downloaded source/manifest/license/README blob hashes were verified.
A temporary external Cargo patch supplied that source only during testing. The committed
git dependency and lockfile retain the pinned source. No replacement trainer implementation
was used. Locally unpacked D-Bus development files/pkg-config and Xvfb dependencies supplied
the build/display environment. An initial missing shared-library symlink caused a link
failure; restoring it allowed the normal shared-library build. No Bluetooth/hardware was exercised.

## Checks executed

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 196 passed, zero failed, across 46 suites. Includes historical
  fixtures, the existing headless/tactical corpora, selection chain and all-zone experiments.
- Final `cargo test -p godot_adapter -p race_engine --test mixed_session --lib`: 23 passed,
  zero failed (15 adapter tests, one existing engine unit test, seven mixed-session tests).
- `cargo test -p race_engine --test mixed_session --release`: seven passed.
- `python3 scripts/check-fixtures.py`: all four frozen regression vectors unchanged.
- `cargo build -p godot_adapter`: passed.
- Godot headless smoke: zero failures, including software power, slot/wheel validation,
  accelerated fixed stepping, export/replay, screen construction, hidden diagnostics and reveal.
- Both `tests/screenshots.tscn` and `tests/mixed_screenshots.tscn` rendered successfully.
  Inspected the workout navigation and public/coaching views at 25:45. Nearby rider labels
  use fixed display size, and the diagnostic panel scrolls within the viewport.

The first fresh Godot import emitted native-class loading errors; the second completed
cleanly, consistent with the existing issue #2. No automatic retry/skip was added to CI.
Headless rendering still emits dummy-renderer null-mesh messages; smoke assertions report
zero failures, and the separate OpenGL screenshots provide actual visual evidence.

## Four complete hour-long comparisons

`cargo run -p race_engine --release --example mixed_session -- mixed-race-output`
executed four 3,600-second sessions, each with 72,000 ordinary kernel steps and 3,600
inspector samples. Percentages below count physically available wheels at 20 Hz.

| Variant | First endurance shelter | Post-chase endurance shelter | Final endurance shelter | Front gap after chase |
|---|---:|---:|---:|---:|
| Prescribed software input | 100.0% | 97.9% | 98.4% | 0.0 m |
| Chase at 204 W instead of 330 W | 100.0% | 0.0% | 0.0% | 187.9 m |
| Exposed slot, prescribed watts | 1.3% | 0.0% | 0.0% | 709.3 m |
| Separately authored declining-pacer plan | 100.0% | 46.2% | 98.4% | 0.0 m |

Prescribed input stayed inside the provisional ±5% FTP band throughout. That is a
controlled-input check, not independent evidence of induced demand. At 1,680 s, the 300 W CP
rider had 14,600 J remaining: independently `20000 - (330 - 300) * 180`. Missing that
contribution preserved reserve but lost the group; the later workload timing stayed unchanged.
The pacer's ordinary input frames remain identical between prescribed and missed-effort runs.
No configuration is rewritten and no free rejoin is inserted.

An initial generic 90% CP spotlight cruise left the warm-up field far behind and was visibly
unsuitable for the fixture. Its fixed own policy now cruises/recovers at 60%/55% CP; capability
and physics remain unchanged. A focused regression requires a nearby field after warm-up.
All final comparison results above use this explicit fixture policy.

## Limits

This is a playable engineering workbench, not the full state graph or validated lesson engine.
Supporting pace was fitted offline against nominal prescribed input. Independent cue-policy
execution, bounded execution acceptance, human recognition/transfer, general team cooperation,
empirical race plausibility and real-trainer race reconciliation remain open. No simulated
samples enter measured activity history. Snapshot schemas and kernel/event contracts are unchanged.
