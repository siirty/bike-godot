# Bike Training (Godot app)

The rider app is a Godot 4 project over the Rust core in `crates/`. Each Rust crate is loaded
in process through the `godot_adapter` GDExtension.

Screens:
- **Workouts**: import ZWO/ERG/MRC files and list them newest first. A workout that can no
  longer be read is still shown.
- **Detail**: the workout's segments and the route it becomes at your FTP.
- **Connect trainer**: scan over BLE, or ride on the simulated trainer.
- **Ride**: a 3D road, a HUD and a route strip. SIM grade is written to the trainer. The
  simulated trainer adds a power slider and a SIM debug panel.
- **Race view**: the deterministic race kernel stepping live, with a scripted player request and reactive spotlight
  opponents (follow/chase/attack/recover/sprint), bounded by CP/W′/Pmax and a feed of the kernel's race events (passes, wheels, finish).
- **Settings**: FTP, saved in `user://settings.cfg`.

The workout library is `user://workouts.sqlite`.

## Run

From the repository root:

```sh
cargo build -p godot_adapter          # the project loads target/debug/libgodot_adapter.*
godot --path apps/rider_godot --import --headless   # first time: builds the class registry
godot --path apps/rider_godot
```

Release builds use `cargo build -p godot_adapter --release`; `bike_core.gdextension` maps both.

## Tests

```sh
godot --headless --path apps/rider_godot res://tests/smoke_test.tscn
```

The smoke test opens a throwaway library and imports a fixture. It rides 20 s on the simulated
trainer at 220 W and steps the race kernel, then exits 0 or 1. `scripts/project-ci.sh` runs it.

`res://tests/screenshots.tscn -- <dir>` saves a PNG of each screen, for layout review. It needs a
display, so do not run it with `--headless`.

## Rules for this folder

GDScript is presentation only. Speeds, distances, grades, formatting and errors come from the
extension already computed and worded. If a script starts calculating something, that logic
belongs in `crates/godot_adapter/src/app/` (plain Rust, unit-tested) or in a domain crate.

The app uses the Compatibility renderer.
