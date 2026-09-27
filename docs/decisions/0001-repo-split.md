# ADR 0001: Repository split — Godot app in a new repo, prototype frozen

- Status: accepted (2026-09-26)
- Deciders: Levin Schmidt (siirty), Hermes

## Context

The Flutter + Dart app (`bike-training-app`) proved the product: ride flow, Victory BLE
pairing, the drafting model, the deterministic race kernel, and — via PR 66 — a Godot race
viewer driven by a replay contract. Continuing meant either growing a Flutter/Dart app whose
end-state is Godot, or porting ~14.8k lines of tested Dart domain code to Rust before
anything user-visible moved.

## Decision

1. **`bike-training-app` is a prototype.** It stays runnable as the reference behavior and
   training oracle, in maintenance mode. Its README and migration doc record this.
2. **New repo `bike-godot`** hosts the Godot app shell and the Rust adapter layer, seeded
   from the template `siirty/template-ai-dev` (AI-workflow structure, quality CI,
   Proxmox/local-runner trigger layer).
3. **The trainer core is not moved** — it already lives in `siirty/bike-training-core` and
   is consumed here as a pinned git dependency (`crates/godot_adapter`).
4. **The Dart domain core is not ported to Rust now.** It stays in `bike-training-app`,
   run headless as a separate process behind the versioned replay/IPC contract. The
   prototype's own architecture doc offered exactly this branch ("keep Dart as a separately
   packaged core with a defined app API").
5. Language split in the new repo: **GDScript** for Godot-native presentation, **Rust** for
   the adapter/services layer. No domain logic in GDScript, ever.

## Consequences

- Fastest path to a publishable MVP: no parity project before first value; the two new
  artifacts are the trainer adapter and the live IPC protocol.
- Two runtimes ship at M2+ (Godot + the Dart core binary): accepted cost, revisited only if
  packaging proves painful or domain feature growth reopens the porting question.
- Golden vectors must be recorded from the Dart kernel (M3) before any future port; the
  prototype remains the oracle and is never deleted.
- The prototype's melos/CI gates are untouched; this repo's CI is independent on the
  self-hosted runner from day one.

## Revisit triggers

A full Rust port of the domain becomes right when any of these hold: domain features grow
actively and the Dart process boundary is daily friction; a target platform makes shipping
the Dart binary painful; or the standalone-networked-device architecture (rust-core.md's
bundler role) makes an in-process Rust kernel necessary.