---
name: ftms-ble-protocol
description: Reference for the Bluetooth FTMS (Fitness Machine Service) protocol — service/characteristic UUIDs, the Indoor Bike Data notification fields, and Control Point opcodes, especially Set Indoor Bike Simulation Parameters (SIM mode). Use whenever implementing or debugging crates/ble_trainer or crates/trainer_control, or anything that talks to a smart trainer over BLE.
---

# FTMS BLE protocol reference

This app uses **FTMS only** (no ANT+), and specifically **SIM mode**, not ERG mode — see
`docs/architecture/physics-design.md` for why. This doc is the protocol-level reference for
implementing that; verify exact byte layouts against the official Bluetooth SIG "Fitness Machine
Service" specification before shipping, the numbers below are for orientation during
implementation, not a substitute for the spec.

## Service & characteristics

Fitness Machine Service (UUID `0x1826`) typically exposes:

- **Fitness Machine Feature** (`0x2ACC`, read) — bitmask of what the trainer supports (power
  measurement, cadence, resistance, simulation, etc.). Read this once on connect to know what's
  usable.
- **Indoor Bike Data** (`0x2AD2`, notify) — the trainer's live telemetry stream.
- **Fitness Machine Control Point** (`0x2AD9`, write + indicate) — how the app commands the
  trainer. Every write is acknowledged via an indication with a response code.
- **Fitness Machine Status** (`0x2ADA`, notify) — out-of-band status changes (e.g. another
  controller, or the trainer itself, changed something).
- **Supported Power Range** / **Supported Resistance Level Range** (read) — validate target
  values against these before writing.

## Indoor Bike Data (`0x2AD2`) — what you read

A leading flags bitmask determines which optional fields follow. Fields commonly present:
Instantaneous Speed, Instantaneous Cadence, Instantaneous Power, Total Distance, Resistance Level,
Heart Rate, Elapsed Time. **Instantaneous Speed is the trainer's own SIM-mode-computed speed** —
in SIM mode, trust this over independently re-deriving the human rider's speed in
`race_physics` (see the design invariant in `docs/architecture/physics-design.md`).

## Control Point (`0x2AD9`) — what you write

Every session needs, in order: **Request Control** (opcode `0x00`) before anything else, then
typically **Start or Resume** (`0x07`) before setting targets. Each write gets an indication back:
**Response Code** (`0x80`) containing the original opcode and a result code — always handle this,
don't assume writes succeed silently.

`trainer_core` is a general-purpose, eventually-open-source FTMS library, not a SIM-only
codec — it implements a broad slice of the Control Point opcode space for its own reusability, not
just what this app calls. **The SIM-only rule for this app is enforced one layer up, at
`crates/ble_trainer`'s worker command set** (SetSimulationParameters / StartOrResume / Pause /
Disconnect, plus scanning) — never an ERG-reachable command. See `docs/architecture/trainer-core.md`
and `.claude/rules/trainer-control-ble.md`. `trainer_core` being *able* to encode ERG is not a
problem; an app-reachable path to it would be.

### Full Control Point opcode set (spec reference)

| Opcode | Name                                             |
| ------ | ------------------------------------------------- |
| `0x00` | Request Control                                   |
| `0x01` | Reset                                             |
| `0x02` | Set Target Speed                                  |
| `0x03` | Set Target Inclination                            |
| `0x04` | Set Target Resistance Level                       |
| `0x05` | Set Target Power (ERG mode)                       |
| `0x06` | Set Target Heart Rate                             |
| `0x07` | Start or Resume                                   |
| `0x08` | Stop or Pause                                     |
| `0x09` | Set Targeted Expended Energy                      |
| `0x0A` | Set Targeted Steps                                |
| `0x0B` | Set Targeted Strides                              |
| `0x0C` | Set Targeted Distance                             |
| `0x0D` | Set Targeted Training Time                        |
| `0x0E`/`0x0F`/`0x10` | Set Targeted Time in Two/Three/Five HR Zones |
| `0x11` | Set Indoor Bike Simulation Parameters (SIM mode)  |
| `0x12` | Set Wheel Circumference                           |
| `0x13` | Spin Down Control                                 |
| `0x14` | Set Targeted Cadence                              |
| `0x80` | Response Code                                     |

As always: verify exact byte layouts against the official Bluetooth SIG spec before shipping — the
notes below are orientation, not a substitute for the spec.

**What `trainer_core` implements**, per its expanded scope as a general-purpose FTMS library:

- **`0x11` — Set Indoor Bike Simulation Parameters (SIM mode, the only one this app
  ever calls)**: 7-byte payload — wind speed, grade, coefficient of rolling resistance (Crr), and
  **wind resistance coefficient (Cw)**. The trainer's own firmware computes resistance and reported
  speed from these plus the rider's real pedaling — this is the entire mechanism behind
  "route/race dynamics without ERG mode." Throttle writes to on-materially-changed-grade
  (~0.2–0.3% delta), ~1 Hz max — see `.claude/rules/trainer-control-ble.md`.

  **Cw is not the same quantity as CdA.** This app's physics model (`race_physics`) works in CdA —
  aerodynamic drag *area* (m²). The FTMS wire field is Cw — wind resistance *coefficient* (kg/m).
  The conversion is `Cw = 0.5 * air_density * CdA` (air_density ≈ 1.225 kg/m³ at sea level). Don't
  write CdA directly into the Cw field, and don't describe them as interchangeable in code or docs.

- **`0x05` — Set Target Power (ERG mode)**: now implemented in `trainer_core` (3-byte payload,
  raw `sint16` watts, no scaling) for library completeness/reusability — a reusable FTMS library
  that can't do ERG isn't very useful to anyone else who might use it. This app never calls it:
  not because the code is incapable of it, but because the app chooses not to, and
  `ble_trainer`'s command set makes sure it can't be invoked by accident either.
- **`0x00` Request Control**, **`0x01` Reset**, **`0x07` Start or Resume**: session lifecycle,
  implemented, needed regardless of ERG/SIM.
- **`0x08` Stop or Pause**: implemented as a 2-byte payload (opcode + one parameter byte,
  `0x01`=Stop / `0x02`=Pause) — this parameter mapping needs verification against the official
  Bluetooth SIG spec, don't treat it as gospel.
- **`0x04` Set Target Resistance Level**: implemented in the spec-correct 3-byte form only. It's a
  raw, trainer-specific percentage with no standardized physical meaning across manufacturers —
  this app avoids it; SIM mode exists precisely so the app doesn't need it.
- **`0x12` Set Wheel Circumference**: implemented — 3-byte payload, `u16` little-endian, 0.1mm
  resolution.
- **`0x80` Response Code**: implemented — 3-byte payload (response opcode marker, the echoed
  request opcode, a result code). Five known result codes: `Success`, `OpCodeNotSupported`,
  `InvalidParameter`, `OperationFailed`, `ControlNotPermitted`.

  **Don't confuse this with opcode `0x05`.** Response Code result-code `ControlNotPermitted` happens
  to be value `0x05` in its own, separate result-code namespace — it has nothing to do with control-
  point opcode `0x05` (Set Target Power/ERG). Same byte value, two unrelated meanings; keep the
  distinction explicit in code and prose.
- Also implemented: **Supported Resistance Range** parsing (reading the trainer's advertised
  min/max/step, not a Control Point opcode itself but part of the same expanded scope).

**Deliberately not implemented** (deferred — real-world evidence from a mature, 100+-device
reference FTMS client shows these are essentially unused by real bike trainers):

- **`0x02` Set Target Speed**, **`0x03` Set Target Inclination**: treadmill-only in practice.
- **`0x06` Set Target Heart Rate**, **`0x09`–`0x10`** (targeted energy/steps/strides/distance/
  training-time/HR-zone-time): not applicable to a bike-trainer use case.
- **`0x13` Spin Down Control**: a stateful multi-step exchange with no reference implementation
  anywhere to verify against — too risky to implement without something to test it against.
- **`0x14` Set Targeted Cadence**: not used.

## Implementation notes

- Wrap all of this behind the `TrainerControl` interface (`crates/trainer_control`). The FTMS/BLE
  I/O itself lives in `trainer_core` (`btleplug`), reached through `crates/ble_trainer` — nothing
  else in the app depends on a BLE library or knows these opcodes exist; the opcodes are
  `trainer_core`'s concern (see `docs/architecture/trainer-core.md`).
- Windows (WinRT) and Linux (BlueZ/D-Bus) are the least battle-tested `btleplug` backends, and
  Android carries the Rust/Java hybrid-build risk — budget explicit testing time, don't assume
  parity with macOS.
- Different trainer manufacturers have shipped firmware with FTMS quirks; validate against ≥2 real
  trainer models before trusting a given control flow (see `docs/architecture/build-order.md`,
  Slice 3).
