---
description: Godot app and GDExtension adapter conventions
paths:
  - "apps/rider_godot/**"
  - "crates/godot_adapter/**"
---

# rider_godot / godot_adapter conventions

- **GDScript is presentation only.** It handles layout, art, camera and input. Speeds, distances,
  grades, formatted text and error wording arrive from the extension already computed. If a script
  starts calculating something, move that logic into `crates/godot_adapter/src/app/` (plain Rust,
  unit-tested without Godot) or into a domain crate.
- **`gd.rs` is a thin binding layer.** It converts between Godot types and `app/` types, and nothing
  else. Only `gd.rs` (and the `entry` module) may touch `godot` types. The `app/` modules stay
  plain Rust.
- **`unsafe` is denied in the adapter** except for the one `#[gdextension]` entry impl.
- **Time comes from Godot's frame loop.** Scripts pass `Time.get_ticks_usec()` (`App.now_usec()`)
  into the controllers, and the Rust side never reads a clock of its own. The race view advances
  the kernel by fixed 50 ms steps from an accumulator, never by a variable `dt`.
- **The ride view shows only what exists.** It shows the human's reported telemetry on the frozen
  route. Do not draw opponents, tactical actions or race outcomes in the ride view until they come
  from the race kernel. The race screen's riders do come from the kernel, and its scripted powers
  are labelled as such.
- **Rides run with the app in the foreground.** Do not chase background BLE.
  `display/window/energy_saving/keep_screen_on` stays enabled (Godot's default).
- **UI layout rules**, which the screenshots caught:
  - Anchor full-rect controls with `set_anchors_and_offsets_preset`, not `set_anchors_preset`.
    The latter keeps stale offsets on a node that has no size yet.
  - Labels wrap only in columns. A wrapping label inside an `HBoxContainer` has no minimum width
    and collapses (`Ui.label` handles this).
  - A container inside a `ScrollContainer` needs `SIZE_EXPAND_FILL` horizontally.
- **Verify every change**:
  - `cargo build -p godot_adapter`;
  - `godot --headless --import --path apps/rider_godot`;
  - `godot --headless --path apps/rider_godot res://tests/smoke_test.tscn`.

  For a visual change, also run `res://tests/screenshots.tscn -- <dir>` with a display and look at
  the PNGs.
- **The Compatibility renderer is deliberate.** It keeps a possible Web export in reach.
