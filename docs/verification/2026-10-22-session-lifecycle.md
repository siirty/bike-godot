# Game session lifecycle verification — 22 October 2026

Base: `a7d412b96712bed1e4ae7de8c1ece43efec65423` (mixed-session prototype).
[ADR 0011](../decisions/0011-game-session-lifecycle.md) records the game rules and
[usage](../architecture/mixed-session-experiment.md) explains controls and report generation.
The world model is provisional. These checks establish software behavior, not empirical realism.

## Executed checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 202 passed, zero failed, across 47 suites.
- Final focused engine/adapter checks: 30 passed (16 adapter, one existing engine unit,
  six lifecycle and seven legacy mixed-session tests). Includes an additional replay test
  through race initialisation, pause, explicit skip and early finish.
- `cargo test -p race_engine --test game_session --release`: six passed.
- `python3 scripts/check-fixtures.py`: all four historical fixtures unchanged.
- `cargo build -p godot_adapter`: passed.
- Godot 4.3 smoke: zero failures. Covers bus-only preparation, race field creation,
  fixed stepping, pause clock hold, incomplete/skipped/abandoned statuses, replay and coaching.
- Full and focused OpenGL screenshots produced. Inspected warm-up/race/cooldown presentation,
  visible controls, result/phase labels and private diagnostic reveal. Bus/stand art is primitive
  prototype presentation, not the planned final art style.
- Twelve complete report runs verified: each has 3,600 one-second samples, 3,600 s of block
  accounting, a frozen outcome at workout 3300 s / race 2700 s, no opponent samples outside Race,
  and eight review checkpoints (96 total). Generated viewer JavaScript passes `node --check`.

Lifecycle tests independently calculate the final warm-up reserve after one second at 600 W:
`20000 - (600 - 300) * 1 = 19700 J`, then verify that exact balance in the initial race state.
NPCs explicitly start fresh. Contrasting zero/600 W cooldowns change player reserve but leave
the competitive state and result exactly unchanged. Paused and finished steps reject without
advancing clocks. Skipped time never becomes executed active time or invented work.

## Game design comparison

Reproduce with:

```sh
cargo run -p race_engine --release --example game_report -- game-session-output
```

Miss holds 204 W for that contribution. Delay holds 204 W for its first 30 s, then follows
the remaining original block; it does not extend hard work into recovery. Overdo requests
115% of the target, still limited by the player's fixed game physiology. Conserve requests
95% of nominal watts in the three endurance blocks. All variants use the identical competitive
plan and capability, then complete cooldown. These are input comparisons, not cue readers.

| Run | Game result | Player front gap at deadline | Post-chase shelter |
|---|---|---:|---:|
| Reference | Success | 4.3 m | 97.9% |
| Miss chase | Failure | 494.9 m | 0.0% |
| Delay chase | Failure | 282.5 m | 1.9% |
| Overdo chase | Failure | 228.9 m | 2.5% |
| Miss tempo | Failure | 233.9 m | 97.9% |
| Delay tempo | Failure | 223.6 m | 97.9% |
| Overdo tempo | Success | 0.0 m | 97.9% |
| Miss short response | Failure | 361.2 m | 97.9% |
| Delay short response | Failure | 291.4 m | 97.9% |
| Overdo short response | Failure | 188.9 m | 97.9% |
| Conserve 5% in endurance | Failure | 1071.2 m | 0.0% |
| Ignore all contributions | Failure | 926.5 m | 0.0% |

The nominal path succeeds and every single missed contribution loses this particular objective.
That does not establish necessity against all strategies. More importantly, 5% easier endurance
loses, and excess chase/response work can also lose: the current frozen pace plan is brittle.
Those are visible design findings, not passed balance gates. The next design iteration should
inspect actual contact/fatigue paths, broaden reasonable execution tolerance and compare public-cue
action policies without adding live target-error rescue. No professional-race or human-learning
prediction claim is made.

## Environment and remaining limits

Linux x86_64, Rust 1.94.0, official Godot 4.3 and Mesa llvmpipe with a TCP Xvfb display.
The private trainer_core dependency was supplied from its exact pinned
`49bde0bd25bed2ce01eb1b1ceb9bb857e5240089` source through a temporary external Cargo patch;
all 23 downloaded source/manifest/license/README Git blob hashes were checked. The committed
git pin and lockfile retain the original source. Native development/display dependencies were
unpacked locally. No physical trainer/Bluetooth was used.

The first fresh Godot import emitted the existing native-class loading issue (#2); a subsequent
import was clean and runtime smoke passed. Headless road meshes emit dummy-renderer null-mesh
messages; the OpenGL render checks are separate. No CI retry or skip was added.
No kernel formula, snapshot schema, RaceEvent contract, historical fixture or measured-activity
path changed. Schema 2 game replay deliberately rejects legacy mixed-v1 traces. Real-trainer
race integration, final art, independent cue execution and world-model verification remain open.
