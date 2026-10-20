# bike-godot

A bike trainer app that turns structured workouts into terrain you ride in FTMS SIM mode. Your
real power decides the outcome; the app never forces watts through ERG mode.

It is a **Godot 4** app over a **Rust core**. Every domain package of the Flutter/Dart prototype
(`siirty/bike-training-app`) has been ported to a Rust crate. Each port is checked bit for bit
against golden vectors recorded from the Dart code. Created from
[`template-ai-dev`](https://github.com/siirty/template-ai-dev).

## Layout

| Path | What |
|---|---|
| `apps/rider_godot/` | The Godot app: workout library, detail and route, trainer connect, ride, race view, settings. GDScript, presentation only. |
| `crates/godot_adapter/` | The GDExtension: thin Godot bindings (`gd.rs`) over plain, tested Rust (`app/`). |
| `crates/workout_import/` | ZWO/ERG/MRC parsing and the versioned segment JSON. |
| `crates/race_physics/` | Forces, power↔speed solvers, fixed-step dynamics, drafting. |
| `crates/session_director/` | Pre-ride terrain synthesis and training contracts. |
| `crates/race_engine/` | The deterministic race kernel, PCG32, snapshot codec, and the workout `RideSession`. |
| `crates/trainer_control/` | The `TrainerControl` interface, SIM write throttle, and simulated trainer. |
| `crates/local_storage/` | The workout library on SQLite (rusqlite). |
| `crates/ble_trainer/` | A SIM-only `TrainerControl` over [`bike-training-core`](https://github.com/siirty/bike-training-core) (pinned git dependency). |
| `fixtures/golden/`, `tools/golden_exporter/` | Parity vectors and the Dart script that records them. |
| `docs/` | [Architecture](docs/architecture/README.md), [decisions](docs/decisions/), [port divergences](docs/port/divergences.md). |

## Quick start

You need Rust (stable) and Godot 4.3 or newer. The private `bike-training-core` dependency is
fetched over git, so configure cargo's git auth if needed (see
[`docs/development/RUNNER_SETUP.md`](docs/development/RUNNER_SETUP.md)).

```sh
cargo build -p godot_adapter
godot --headless --import --path apps/rider_godot
godot --path apps/rider_godot
```

To ride without hardware, choose **Ride without a trainer (simulate)** and use the power slider.

## Checks

```sh
./scripts/project-ci.sh     # fmt, clippy -D warnings, tests, extension build, Godot smoke test
```

The Godot steps run when `godot` is on PATH or `GODOT` points at a binary.

## Re-recording parity vectors

Do this only to re-check parity; the Dart prototype is frozen. The steps are in
[`tools/golden_exporter/README.md`](tools/golden_exporter/README.md).

## Runner

CI runs on the self-hosted `gh-runner` (labels `[self-hosted, linux, X64]`) via the
`CI_RUNNER_JSON` repository variable. Optional disposable Proxmox CI is wired through
`local-proxmox-ci.yml` after `scripts/setup-local-ci-labels.sh`.
