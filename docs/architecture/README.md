# Architecture

The product is a **Godot app** composed of three separately-owned pieces, joined at two
deliberate seams. Decision record: `docs/decisions/0001-repo-split.md`.

```
┌─────────────────────────── this repo (bike-godot) ───────────────────────────┐
│  apps/race_viewer_godot        GDScript — scenes, camera, HUD, menus        │
│        │  replay/IPC (JSON frames; file today, live pipe/socket at M2)       │
│        ▼                                                                     │
│  crates/godot_adapter  ── GDExtension ── trainer_core (git dep)              │
└──────────────────────────────────────────────────────────────────────────────┘
                    │                                    │
                    ▼                                    ▼
   Dart domain core (separate process,        bike-training-core repo
   headless: race_engine, race_physics,       (Rust; BLE/FTMS owner;
   session_director, workout_import)          unchanged, versioned independently)
```

## Language split (deliberate)

- **GDScript** — everything Godot-native. Fast iteration; PR 66's viewer already lives here.
- **Rust** — the adapter layer and any native services. Proven pattern from the prototype's
  flutter_rust_bridge chokepoint; static typing where bugs are expensive.
- **Dart** — the domain kernel, unchanged, behind a process boundary. It is the tested
  specification of race physics/drafting/determinism; porting it is explicitly deferred and
  must be incremental, feature-by-feature, each gated on golden vectors recorded from the
  Dart reference (M3).

## The two seams

1. **Domain seam** — the Dart kernel becomes a standalone process speaking the versioned
   replay contract (already proven by PR 66's file-based export), upgraded to live
   step/telemetry IPC at M2. The kernel stays free of Godot nodes; Godot only interpolates.
2. **Trainer seam** — `crates/godot_adapter` binds `trainer_core`'s `TrainerConnection`
   contract for Godot. SIM-mode-safe subset only; simulated trainers stay available for
   development; ERG/target-power writes are never routed.

## Boundary rules (inherited from the prototype's architecture doc)

- The domain decides movement, physics, tactics, outcomes. Godot owns presentation only.
- The presentation projection carries only visible positions/speeds/event labels; it cannot
  resume the kernel.
- Record golden trajectories and seed streams before any language port. Never maintain two
  production kernels: the Dart prototype's kernel is the reference until (and unless) a port
  is explicitly gated in.
- Do not claim native trainer/BLE or cross-platform support before each is proven on the
  real ride computer (Victory Linux flow first).