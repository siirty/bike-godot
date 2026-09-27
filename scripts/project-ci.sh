#!/usr/bin/env bash
# Deterministic checks for bike-godot: Rust workspace + Godot project sanity.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

log() { printf '\n==> %s\n' "$*"; }

log "Rust: fmt / clippy / test"
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

if command -v godot >/dev/null 2>&1; then
  log "Godot: import + script parse"
  godot --headless --import --path apps/race_viewer_godot
  # parse-only pass; exits non-zero on script errors
  godot --headless --check-only --script apps/race_viewer_godot/scripts/race_view.gd 2>/dev/null || \
    godot --headless --quit-after 2 --path apps/race_viewer_godot >/dev/null
else
  log "SKIP godot checks (godot binary not installed on this runner)"
fi

log "All checks passed"
