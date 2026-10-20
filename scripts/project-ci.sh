#!/usr/bin/env bash
# Deterministic checks for bike-godot: the Rust workspace, then the Godot app end to end.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

log() { printf '\n==> %s\n' "$*"; }

log "Rust: fmt / clippy / test"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

GODOT="${GODOT:-$(command -v godot || true)}"
if [[ -n "$GODOT" ]]; then
  log "Godot: build extension, import, smoke test"
  # The project loads target/debug/libgodot_adapter.* (see apps/rider_godot/bike_core.gdextension).
  cargo build -p godot_adapter
  # --import builds the class_name registry the scripts depend on.
  "$GODOT" --headless --import --path apps/rider_godot
  # Runs as a scene so the App autoload is present; exits 1 on any failed check.
  "$GODOT" --headless --path apps/rider_godot res://tests/smoke_test.tscn
else
  log "SKIP Godot checks (no godot binary on PATH and GODOT is unset)"
fi

log "All checks passed"
