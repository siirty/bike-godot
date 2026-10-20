---
description: TrainerControl, FTMS, and the SIM-only boundary over trainer_core
paths:
  - "crates/trainer_control/**"
  - "crates/ble_trainer/**"
  - "tools/ftms-spike/**"
---

# trainer_control / ble_trainer conventions

- **All ride-loop code depends on the `TrainerControl` trait** (`crates/trainer_control`), never on
  `ble_trainer` or `trainer_core` directly. That is what lets tests and CI exercise the full ride
  loop against the simulated trainers, with no real hardware. See
  `docs/architecture/trainer-core.md`.
- **`TrainerControl` is synchronous with explicit time.** Every call takes a monotonic `now`, and
  `poll(now)` returns events. Never add a sleep, a timer, `Instant::now()` or a blocking wait
  behind it. Async BLE work runs on `ble_trainer`'s tokio worker, and `poll` only drains what has
  already arrived.
- **FTMS SIM mode, not ERG mode.** Write grade, wind, Crr and Cw through the Indoor Bike Simulation
  Parameters control point. Never add a path that writes a fixed target-power control point (ERG):
  that is the exact mechanic this app exists to avoid.

  The chokepoint is `ble_trainer`'s worker command enum, plus the scanner. The
  `sim_only_surface` test scans the crate for ERG symbols; do not weaken it. `trainer_core` may
  implement the ERG encoder as a general-purpose library function, but it must never be reachable
  from the app. `tools/ftms-spike` is held to the same SIM-only rule.
- **SIM-parameter writes are throttled**: only on a materially changed grade (~0.2–0.3 % delta), at
  ~1 Hz max. Hammering the control point can overwhelm some trainer firmware. **That policy has
  exactly one owner: `ThrottledTrainerControl`**, a `TrainerControl` decorator. Compose it. Do not
  re-implement the rule in a caller, and do not add a second throttle beside it.

  It lives in `trainer_control` so that one implementation covers both the real and the simulated
  trainer. That is what makes CI's ride-loop tests actually exercise it. `trainer_core`'s
  `MIN_SIM_PARAMS_WRITE_INTERVAL` *delays* rather than drops writes. It is a backstop, not the
  policy.
- **BLE I/O, the physics tick and the render loop are independent.** Never block one on another.
- **Treat Linux (BlueZ/D-Bus) and Windows (WinRT) as the highest-risk BLE platforms.** Android is
  the highest-risk *build*: the `btleplug` Rust/Java hybrid plus R8 keep rules. Do not assume
  behavioral or build parity across platforms without testing on the actual platform. See
  `docs/architecture/risks.md`.
- **Record real-hardware findings in `docs/trainers/`**, never only in code comments.
