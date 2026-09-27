# bike-godot

Godot shell for the bike trainer app. Created from
[`template-ai-dev`](https://github.com/siirty/template-ai-dev).

## Decision context

The Flutter + Dart app (`siirty/bike-training-app`) is now the **prototype**: it stays
runnable as the reference behavior and training-oracle, but new product work happens here.
This repo is a **composition project**, not a port:

| Piece | Where it lives | Role |
|---|---|---|
| Godot app shell | this repo, `apps/` | UI, camera, HUD, menus, IPC client. GDScript. |
| Trainer core | [`siirty/bike-training-core`](https://github.com/siirty/bike-training-core) | Rust crate owning BLE/FTMS I/O. Unchanged, consumed as a git dependency. |
| Domain core | `siirty/bike-training-app` (`packages/race_engine`, `race_physics`, `session_director`, `workout_import`) | Stays **Dart**, run headless as a separate process behind the replay/IPC protocol. No language port. |

The full rationale — why the Flutter app became a prototype, why the Dart domain is not
being ported to Rust for now, and the language split — lives in
[`docs/architecture/README.md`](docs/architecture/README.md).

## Layout

- `apps/race_viewer_godot/` — seed imported from PR 66 of `bike-training-app`
  (deterministic-kernel replay viewer, perspective chase camera, low-poly art).
- `crates/godot_adapter/` — Rust GDExtension adapter over `trainer_core`
  (SIM-safe subset only: scan / connect / telemetry / SIM params — never ERG routing).
- `docs/decisions/` — ADRs for the repository split and language choices.

## First milestones

1. **M1 — Repo + CI green**: this skeleton, quality CI on the self-hosted runner.
2. **M2 — Live ride**: trainer connect via the adapter, a rideable workout, live rider
   position in the Godot view (replay contract becomes live IPC). The gate for the whole
   architecture: proven on the Linux ride computer with the real trainer.
3. **M3 — Golden-vector harness**: record trajectories/seed streams from the Dart
   prototype; compare against any future domain port.
4. **M4 — Product shell**: workout import UI, persistence, packaging.

## Runner

CI runs on the self-hosted `gh-runner` (labels `[self-hosted, linux, X64]`) via the
`CI_RUNNER_JSON` repository variable; optional disposable Proxmox CI is wired through
`local-proxmox-ci.yml` after `scripts/setup-local-ci-labels.sh`.