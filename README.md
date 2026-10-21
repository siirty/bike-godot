# bike-godot

A bike trainer app that turns structured workouts into credible race situations, teaching when
to commit power and how to classify moves. Your real effort decides the consequences in FTMS SIM
mode; the app never forces watts through ERG mode. The accepted product behavior is defined by
[TT-01–TT-10](docs/requirements/tactical-training.md); implementation is still in progress.

It is a **Godot 4** app over a **Rust core**. Build, tests, fixture maintenance and simulation
development are owned by this repository. No legacy application checkout or language toolchain is
required. Created from [`template-ai-dev`](https://github.com/siirty/template-ai-dev).

## Layout

| Path | What |
|---|---|
| `apps/rider_godot/` | The Godot app: workout library, detail and route, trainer connect, ride, race view, settings. GDScript, presentation only. |
| `crates/godot_adapter/` | The GDExtension: thin Godot bindings (`gd.rs`) over plain, tested Rust (`app/`). |
| `crates/workout_import/` | ZWO/ERG/MRC parsing and the versioned segment JSON. |
| `crates/race_physics/` | Forces, power↔speed solvers, fixed-step dynamics, drafting. |
| `crates/session_director/` | Pre-ride terrain synthesis and training contracts. |
| `crates/race_engine/` | The deterministic race kernel, spotlight tactics, PCG32, snapshot codec, and workout `RideSession`. |
| `crates/trainer_control/` | The `TrainerControl` interface, SIM write throttle, and simulated trainer. |
| `crates/local_storage/` | The workout library on SQLite (rusqlite). |
| `crates/ble_trainer/` | A SIM-only `TrainerControl` over [`bike-training-core`](https://github.com/siirty/bike-training-core) (pinned git dependency). |
| `fixtures/golden/` | Frozen, self-contained regression vectors with provenance and checksums. |
| `tools/ftms-spike/` | SIM-mode experiments against a real trainer; results go into [`docs/trainers/`](docs/trainers/). |
| `docs/` | [Architecture](docs/architecture/README.md) (design, build order, open questions), [decisions](docs/decisions/), [trainer datasheets](docs/trainers/), [port divergences](docs/port/divergences.md). |

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

## Regression fixtures

The checked-in vectors are frozen historical evidence, not generated expectations. See
[`fixtures/golden/README.md`](fixtures/golden/README.md). New behavior is specified here and tested
with independent calculations, state transitions and deterministic kernel runs. Never regenerate
expected results from the implementation under test.

## Runner

CI runs on the self-hosted `gh-runner` (labels `[self-hosted, linux, X64]`) via the
`CI_RUNNER_JSON` repository variable. Optional disposable Proxmox CI is wired through
`local-proxmox-ci.yml` after `scripts/setup-local-ci-labels.sh`.

## All-zone race experiment

The [race transition laboratory](tools/race-prototype/README.md) tests a frozen opponent-action
schedule across Z1–Z7 and recovery. It exports browser playback and machine-readable diagnostics:

```sh
cargo run -p race_prototype --release -- --out race-prototype-output
```

This is an experimental frozen-command policy for the director-first direction in
[ADR 0007](docs/decisions/0007-tactical-training-core.md). The version-3 viewer adds public roster/context, a cue-driven rider, unscored classification
practice and separate audit evidence. Its provisional formation gates are not
validated learning or result-plausibility criteria. Nominal fitting and deviation failures remain
inspectable; the tool does not implement all TT-01–TT-10 requirements.

A focused [connected selection experiment](docs/decisions/0009-cue-calibrated-selection-chain.md)
adds offline cue-policy fitting, local group roles, a lateral-file playback and explicit plan
rejection. Run `cargo run -p race_prototype --release --example chain -- connected-selection.json`.
[Verification](docs/verification/2026-10-21-selection-chain.md) records the accepted bounds and
failed profiles; this is not a general workout generator or human-learning validation.
