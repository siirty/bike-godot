# Risks to budget for

These risks are named up front so they are budgeted for rather than discovered mid-implementation.

- **Linux BLE (BlueZ/D-Bus)** is the dogfooding platform and historically the flakiest. Expect
  polkit permissions, session versus system bus, and BlueZ version skew across distros. Budget
  explicit QA time.
- **Windows BLE (WinRT)** is the newest and least battle-tested of `btleplug`'s backends. Treat it
  as an explicit spike, not an assumption.
- **The Android build.** `btleplug` on Android is a hybrid Rust/Java build: a `droidplug` `.aar`
  alongside the Rust `.so`. Here that `.so` is also the GDExtension, loaded by a Godot Android
  export.

  It needs R8 keep rules for `com.nonpolynomial.**` and `io.github.gedgygedgy.**`. Without them, R8
  strips classes that only native code reaches, and the app crashes at BLE connect with
  `ClassNotFoundException` / `FutureException` (btleplug issue #311). Budget the export wiring as
  its own spike.
- **No BLE hardware in CI, ever.** This is why the simulated trainers exist behind
  `TrainerControl`. They are the only way CI and agents can exercise the ride loop at all.
- **Per-brand SIM-mode quirks** are invisible to CI: gradient caps, parameter units, resistance
  smoothing and latency. The JetBlack Victory spike records them in [`docs/trainers/`](../trainers/).
- **Write cadence.** Hammering the FTMS control point can overwhelm trainer firmware. The policy has
  one owner, `ThrottledTrainerControl`; see [trainer-core.md](trainer-core.md).
- **Concurrency.** BLE I/O runs on `ble_trainer`'s tokio worker. The physics and render loops are
  driven by Godot's frame. Never block the frame on BLE: `poll(now)` only drains what has already
  arrived.
- **Background BLE is avoided by design.** Rides need the screen on anyway, for the road view and
  the HUD, so the app stays in the foreground. Godot's `display/window/energy_saving/keep_screen_on`
  (default on) keeps the display awake. Do not turn it off.
- **`.fit` binary parsing** (Garmin's format, used for workouts and GPS courses) is the highest-risk
  parser. Confirm a usable Rust crate exists before committing to it, or plan to port the decode
  logic. See `.claude/skills/workout-file-formats/SKILL.md`.
- **The GDExtension on every export target.** Each platform needs the adapter library
  cross-compiled for it. A Web export would need gdext's experimental Wasm support plus a non-BLE
  trainer path. See [platform-rollout.md](platform-rollout.md).
- **No local Mac hardware.** iOS and macOS cannot be built or tested locally.
