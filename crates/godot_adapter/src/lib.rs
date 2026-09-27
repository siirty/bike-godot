//! Godot-facing adapter over `trainer_core`.
//!
//! Owns the BLE/FTMS lifecycle the Godot shell needs: scan, connect,
//! telemetry, SIM-mode parameter writes. Deliberately excludes ERG/target
//! power routing, mirroring `.claude/rules/trainer-control-ble.md` in
//! bike-training-app. The GDExtension surface lands in the first
//! implementation slice; this crate exists so the dependency graph and the
//! SIM-safe boundary are visible from day one.
