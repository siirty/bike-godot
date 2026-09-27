# Godot race viewer (migration prototype)

This is a separate Godot 4 app. It renders a rolling road and every rider in a recorded
`RaceSimulation` state. The viewer does **not** calculate speeds, award positions, or control a
trainer. Its only authoritative input is the race core's exported replay.

## Run on Linux

From the repository root, after `melos bootstrap` (the generated Freezed Dart sources are
required):

```sh
dart run packages/race_engine/tool/export_godot_replay.dart
godot --path apps/race_viewer_godot
```

The first command writes `apps/race_viewer_godot/data/race_replay.json`. Without it, the Godot
scene opens in a clearly labelled **visual preview** with synthetic rider positions, to allow
reviewing art and camera without a Dart toolchain. Press Space to pause, left/right arrows to
scrub the recorded race, or R to restart. The preview never represents itself as a live race.

To check the road markings and camera placement without opening a graphical window:

```sh
godot --headless --path apps/race_viewer_godot --script res://tests/road_geometry.gd
```

Godot uses its Compatibility renderer here to retain a Web export option. The generated JSON
is a presentation projection, not the deterministic continuation snapshot: it intentionally
excludes random state, rider capabilities, and privileged simulation inputs. Do not resume a
simulation from it. The producer can later become a live transport without changing the scene's
rendering API.

## Status

- The exporter steps the existing Dart race kernel with fixed, explicitly supplied example
  rider powers. These are scripted inputs for a replay, not autonomous tactical AI.
- Trainer/BLE connection, workout selection, persistence, and human telemetry still live in the
  Flutter app. They have not been migrated.
- Replacing Flutter completely also requires either porting the Dart simulation and workout
  packages with parity tests, or retaining a Dart core runtime. Rust currently owns the separate
  trainer sensor core, **not** the race simulation.
- Linux is the initial visual target. Android, iOS, macOS, and Web need native builds and
  platform verification before this viewer can replace the app.
