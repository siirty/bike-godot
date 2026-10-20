# Trainer datasheets

Hardware-ground-truth records for the smart trainers this project actually drives. This
directory is the single home for per-trainer facts that no CI run can produce: what the
trainer accepts over FTMS, where it deviates from the spec, and how it behaves over time.

## Why this exists

Per-brand SIM-mode quirks (gradient caps, param units, resistance smoothing/latency,
control-point behavior) are invisible to CI — only real hardware can answer them. Every
claim here comes from either (a) official manufacturer documentation, (b) a repeatable
hardware experiment run on the actual unit, or (c) a named third-party source. Anything
not verified on *our* hardware is marked **unverified** — plausibly correct, never
trusted until the spike confirms it.

- The JetBlack Victory sheet and this category came from the SIM-mode spike
  (`siirty/bike-training-app#50`); its experiments are `tools/ftms-spike`.
- The blank [`TEMPLATE.md`](TEMPLATE.md) is copied per trainer. Fill only what is
  known; leave unknowns as `unverified` rather than guessing.
- Update a sheet whenever firmware changes behavior — sheets are per-firmware records,
  not per-model marketing copies.

## Document lifecycle

- Datasheets are **records, not requirements**: they inform adapter design and physics
  assumptions but never prescribe behavior. Requirements live in `docs/architecture/`
  and the slice issues.
- A quirk observed on hardware supersedes any spec-derived or third-party claim in the
  same field — mark the superseded source, don't delete it.
- A trainer is **dogfood-eligible** only when its sheet has no unverified entry in the
  control path (grade/wind/Crr/Cw writes, handshake, telemetry) that Slice 9's adapter
  depends on.
- The §"Physics model inputs" section of each sheet is the contract with
  `physics-design.md`: when a sheet changes, check whether the physics assumptions it
  feeds still hold.

## Sheets

| Trainer | Firmware | Status | Sheet |
|---|---|---|---|
| JetBlack Victory | 4.22 (target) | spike in progress | [`jetblack-victory.md`](jetblack-victory.md) |