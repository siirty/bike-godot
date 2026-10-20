# JetBlack Victory trainer datasheet

Pre-filled for the SIM-mode spike (`siirty/bike-training-app#50`) from official specs, cross-checked third-party sources, and the
QZ (qdomyos-zwift) codebase. Control-path and response fields are **unverified** until
the hardware session runs — that is the spike's remaining work, not a gap in the record.

## Identity

| Field | Value |
|---|---|
| Brand / model | JetBlack Victory (direct drive, Zwift Cog / cassette) |
| Trainer type | Direct drive |
| Control protocols | Bluetooth FTMS, ANT+ FE-C (official; no proprietary protocol needed) |
| Firmware at record | 4.22 is the latest public release (July 2025); **target firmware for the spike** |
| Record status | Spike in progress — pre-hardware |
| Evidence sources | JetBlack support docs; DC Rainmaker review (2024-11-14); GPLama review (2024-10-08); qdomyos-zwift `ftmsbike` + issue #4563 |

## Official specifications (manufacturer)

| Field | Value | Source |
|---|---|---|
| Max simulated gradient | **16%** | JetBlack support: "What are the VICTORY Smart Trainer technical specifications?" |
| Max power | 1800 W | JetBlack support specs page |
| Stated power accuracy | ±2% | JetBlack support specs page |
| Control channels | Bluetooth FTMS + ANT+ FE-C | JetBlack support specs page |
| Max rider weight | 120 kg | JetBlack support specs page |
| Mains power required | Yes — resistance control requires external power | DC Rainmaker review |

## Control path (FTMS SIM mode)

| Field | Value |
|---|---|
| Handshake (Request Control → Start/Resume) | unverified |
| SIM params write (opcode 0x11) accepted | unverified — QZ treats it as a standard FTMS bike; no Victory-specific write quirk known upstream |
| Grade wire units observed | unverified — expected per spec sint16 LE @0.01 %/LSB; matches QZ's encoder exactly |
| Wind wire units observed | unverified — expected per spec sint16 LE @0.001 m/s/LSB, signed (head +, tail −) |
| Crr / Cw units observed | unverified — expected per spec u8 @0.0001 and @0.01 kg/m |
| Grade cap behavior | unverified — cap is 16% per official specs; behavior above cap (clamp / ignore / error) not documented anywhere public |
| Control-point rejection quirks | unverified — test for D500V2-style "0x11 before 0x07" rejection (seen on other brands in QZ) |
| Out-of-handshake write behavior | unverified |
| Minimum write interval observed | unverified — app policy stays ~1 Hz max, on materially-changed grade (`ThrottledTrainerControl`) |

## Grade response

| Field | Value |
|---|---|
| Latency: grade write → resistance change | unverified — no public latency numbers for SIM-mode grade steps |
| Smoothing | unverified — DCR's SIM-mode tracks (Sand and Sequoias) tracked reference power "very closely" with a 1-second overestimate on one sprint; no smoothing constants published |
| Behavior above grade cap | unverified — spike experiment 1 |
| Behavior at 0% / negative grade | unverified — official range starts at 0%; negative-grade handling undocumented |

## Telemetry

| Field | Value |
|---|---|
| Power reporting | ⚠️ ERG-mode power smoothing **cannot be disabled**; reported watts are "artificially perfect (flat), often just reporting what you told it" (DC Rainmaker). SIM-mode behavior unverified — test explicitly |
| Cadence / speed | unverified |
| Coast-down behavior | ⚠️ **Known flaw (GPLama, Oct 2024, firmware 4.x-era):** after ~10 min efforts with coasting/descents, reported power floors around 25 W instead of dropping to 0 until a manual spindown calibration; bad enough to shelve the trainer for racing. Fix status in current firmware unknown — verify early |
| Notification rate (BLE) | unverified — WiFi "Race Mode" gives faster power updates; BLE FTMS rate is the number the ride loop actually sees |
| Reported vs `powerForSpeed` divergence | unverified — feeds Slice 9 measured-vs-simulated separation design |

## Physics model inputs

| Field | Value |
|---|---|
| Effective grade cap for terrain generation | 16% (official) — matches `trainer_core` `device_quirks::DEFAULT_MAX_GRADE_FRACTION = 0.16`; engine's generated grade band [-12%, +15%] stays inside it |
| Crr / CdA defaults validated | unverified |
| Wind offset usable as designed | unverified (units spec-consistent) |
| Known deviations from `physics-design.md` assumptions | none yet recorded |

## Quirks & deviations (running list)

- ERG-mode watts are firmware-smoothed and cannot be disabled (DC Rainmaker, fw 4.x, 2024-11) — if SIM-mode telemetry inherits this, trainer-reported power is filtered, not raw.
- Coast-down power floor after sustained efforts pending recalibration (GPLama, 2024-10, fw 4.x-era) — current-firmware status unverified.
- Firmware update path is Bluetooth-only via the JetBlack app; users report confusing version jumps (4.5 → 4.13 → 4.22) and flaky WiFi setup on dual-band routers (Zwift forums, r/JetBlack). Pin the exact firmware in the verification log.
- QZ issue #4563 (resistance "very hard or very easy") went stale-closed unresolved; the debug log showed *no QZ→trainer commands at all*, pointing to an app-side pairing problem rather than a Victory SIM quirk — weak signal, do not treat as a trainer fault.

## Open questions

- What does the trainer do above 16%: clamp, ignore, or respond with an error code?
- Is SIM-mode telemetry smoothed like ERG mode, and can it be disabled?
- Does the Victory accept 0x11 without a preceding Start/Resume, or reject like the D500V2?
- What is the grade-write → resistance-change latency and ramp shape at race-step cadence?
- Does coast-down reach 0 W cleanly on current firmware, or does the GPLama floor persist?
- What BLE notification rate does Indoor Bike Data sustain during SIM mode?

## Running the spike

The experiments are committed and repeatable — `tools/ftms-spike` (a throwaway workspace
member crate that depends only on `trainer_core`, never on the app's crates):

```sh
cargo run -p ftms_spike -- --exp ramp       # experiments 1: grade ramp incl. cap probe
cargo run -p ftms_spike -- --exp units      # experiment 2: wind sign, Crr/Cw steps
cargo run -p ftms_spike -- --exp latency    # experiment 3: step response timing
cargo run -p ftms_spike -- --exp sustain    # experiment 4: ~10 min profile + coast-down
cargo run -p ftms_spike -- --exp telemetry  # experiment 5: reported vs model power
```

Defaults target the Victory (`--name VICTORY` filter over BlueZ); `--sim` rehearses any
experiment against the core's simulated trainer with no hardware. Output rows paste into
the verification log below. Fill rows **during** the session — felt resistance is data
only the rider can record.

## Verification log

| Date | Firmware | Experiment | Result | Evidence |
|---|---|---|---|---|
| 2026-09-21 | — | Desk research (specs + QZ cross-check) | Cap 16% confirmed official; wire units spec-consistent vs QZ encoder; SIM latency, cap-overshoot behavior, telemetry smoothing still open | this sheet's sources; bike-training-app#50 |
| — | 4.22 (target) | Spike experiments 1–5 (grade ramp, units, latency, sustained ride, telemetry sanity) | pending hardware session | — |