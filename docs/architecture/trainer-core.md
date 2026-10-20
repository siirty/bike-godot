# Trainer core boundary

BLE and FTMS live in [`siirty/bike-training-core`](https://github.com/siirty/bike-training-core),
as the `trainer_core` crate. This workspace consumes it as a Cargo git dependency, pinned by commit
rev in the root `Cargo.toml`. The repository is private, so CI needs read access to it; see
[`docs/development/RUNNER_SETUP.md`](../development/RUNNER_SETUP.md).

`trainer_core` covers:
- the FTMS wire codec;
- Bluetooth LE via `btleplug`: scan, connect, GATT discovery, the connect handshake, telemetry
  decoding and Control Point writes;
- a simulated connection.

Its own README and module docs give the exact scope. The name is transport-neutral on purpose. A
WiFi transport (FTMS "Direct Connect" over TCP) can land beside `ble` as another implementation of
the same `TrainerConnection` contract.

## Layering

```
apps/rider_godot ── godot_adapter ── TrainerControl (trainer_control)
                                         ├─ SimulatedTrainer           (race_physics solver, no BLE)
                                         ├─ CoreSimulatedTrainer       (ble_trainer → trainer_core simulator)
                                         └─ BleTrainer                 (ble_trainer → trainer_core BLE)
```

Every ride-loop consumer depends on `TrainerControl`, never on `ble_trainer` or `trainer_core`
directly. This is what lets CI and agents exercise the full ride loop with no hardware, and it is
what makes the implementation underneath swappable.

`TrainerControl` is synchronous and takes explicit time. Every call takes a monotonic `now`, and
`poll(now)` returns connection-state and telemetry events. `ble_trainer` bridges that interface to
the async core:
- it runs one tokio worker task per trainer;
- commands go to the worker over a channel;
- telemetry is drained on the next `poll`.

## The SIM-only invariant

The app never writes a fixed-power ERG target. `trainer_core` does implement ERG (`Set Target
Power`, opcode `0x05`) and other control-point opcodes, because it is a general-purpose FTMS
library, and a reusable FTMS library that cannot do ERG is useless to anyone else. That is
deliberate, and it is not a violation.

The invariant is enforced at the chokepoint: `ble_trainer`'s worker command enum. It has exactly
these commands:
- `SetSimulationParameters`;
- `StartOrResume`;
- `Pause`;
- `Disconnect`.

The scanner adds start / poll / stop / connect. Nothing in `ble_trainer` reaches an ERG opcode, and
the `sim_only_surface` test scans the crate's source for ERG symbols. When reviewing `ble_trainer`
changes, check that command set. Whether `trainer_core` *can* do ERG is not the question.

## The write-rate policy

SIM parameter writes are throttled: only on a materially changed grade (~0.2–0.3 % delta), at
~1 Hz max. Hammering the control point can overwhelm trainer firmware. The policy has exactly one
owner, `ThrottledTrainerControl` in `trainer_control`, a `TrainerControl` decorator. It lives above
both the simulated and the real trainer, so CI's ride-loop tests exercise it. `trainer_core`'s
`MIN_SIM_PARAMS_WRITE_INTERVAL` *delays* rather than drops writes. It is a backstop, not the policy.

## Not yet done

- **Real-hardware validation.** The BLE path has run against a fake connection in tests, but never
  against a physical trainer. The JetBlack Victory spike comes first; see
  [product-alignment.md](product-alignment.md).
- **The sensor-bundler role.** Fusing a standalone heart-rate monitor, power meter or
  speed/cadence sensor into the telemetry frame is undesigned. When it lands, `TrainerTelemetry`
  grows optional fields and stays the boundary type.
- **Standalone mode.** The same core could run on a Pi-class board and stream over the network.
  The wire schema is deferred until that mode actually ships. Until then, the boundary is the Rust
  types in `ble_trainer`, so there is no second source of truth.
