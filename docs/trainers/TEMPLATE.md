# `<brand> <model>` trainer datasheet

Copy this file per trainer (e.g. `jetblack-victory.md`) and fill it from official specs,
hardware experiments, and named third-party sources. **Fill only what is known** — every
field you cannot verify stays `unverified`. A datasheet full of `unverified` is a good
datasheet; a guessed value is a bug in the record.

## Identity

| Field | Value |
|---|---|
| Brand / model | |
| Trainer type | (direct drive / wheel-on / smart bike) |
| Control protocols | (FTMS / ANT+ FE-C / proprietary — list what the trainer actually exposes) |
| Firmware at record | (version + date; sheets are per-firmware records) |
| Record status | (spike in progress / datasheet current / superseded by firmware X) |
| Evidence sources | (official doc / our spike run / third-party, named) |

## Official specifications (manufacturer)

| Field | Value | Source |
|---|---|---|
| Max simulated gradient | | |
| Max power | | |
| Stated power accuracy | | |
| Control channels | | |

## Control path (FTMS SIM mode)

| Field | Value |
|---|---|
| Handshake (Request Control → Start/Resume) | |
| SIM params write (opcode 0x11) accepted | |
| Grade wire units observed | (expected per spec: sint16 LE, 0.01 %/LSB) |
| Wind wire units observed | (expected per spec: sint16 LE, 0.001 m/s/LSB, signed head+/tail−) |
| Crr / Cw units observed | (expected per spec: u8, 0.0001 / 0.01 kg/m) |
| Grade cap behavior | (value; clamps / ignores / error above cap?) |
| Control-point rejection quirks | (response codes for unsupported ops?) |
| Out-of-handshake write behavior | (accepted without re-requesting control?) |
| Minimum write interval observed | (app policy: ~1 Hz max, on materially-changed grade) |

## Grade response

| Field | Value |
|---|---|
| Latency: grade write → resistance change | |
| Smoothing | (visible ramp? time constant?) |
| Behavior above grade cap | |
| Behavior at 0% / negative grade | |

## Telemetry

| Field | Value |
|---|---|
| Power reporting | (raw or smoothed — can smoothing be disabled?) |
| Cadence / speed | (plausibility check vs app model) |
| Coast-down behavior | (power → 0 cleanly, or floor artefact?) |
| Notification rate (BLE) | |
| Reported vs `powerForSpeed` divergence | (per grade; feeds Slice 9 separation design) |

## Physics model inputs

| Field | Value |
|---|---|
| Effective grade cap for terrain generation | |
| Crr / CdA defaults validated | |
| Wind offset usable as designed | |
| Known deviations from `physics-design.md` assumptions | |

## Quirks & deviations (running list)

- (one line per quirk: what happens, conditions, evidence)

## Open questions

- (what the next hardware session must answer)

## Verification log

| Date | Firmware | Experiment | Result | Evidence |
|---|---|---|---|---|
| | | | | |