---
name: cycling-physics
description: Reference for the cycling power/speed physics formulas and typical parameter values (Crr, CdA, air density, drivetrain efficiency) and standard %FTP training zones. Use whenever implementing or debugging crates/race_physics, or crates/session_director's terrain synthesis.
---

# Cycling physics reference

Full design context (why a numeric solver, what it's for vs. what the trainer itself computes) is
in `docs/architecture/physics-design.md`. This doc is the formula/constants reference.

## Power ↔ speed

Forward direction (closed-form, `v` known → `P(v)`):

```
P(v) = [ m·g·(sinθ + Crr·cosθ) + 0.5·ρ·CdA·|v + w|·(v + w) ] · v / η
```

- `m` — combined rider + bike mass (kg)
- `g` — 9.80665 m/s²
- `θ` — `atan(grade)`, where `grade` is rise/run (e.g. 0.06 for 6%)
- `Crr` — coefficient of rolling resistance
- `ρ` — air density (kg/m³)
- `CdA` — drag area (m²)
- `v` — rider ground speed (m/s); `w` — headwind component (m/s, positive = headwind)
- `η` — drivetrain efficiency

Inverse direction (`P` known → solve `v`) has no closed form because of the cubic aero term — use
a safeguarded Newton–Raphson with bisection fallback, bracketed to `v ∈ [0, ~30 m/s]`; `P(v)` is
monotonic increasing for physically sane inputs so this converges in single-digit iterations. See
`crates/race_physics/src/solver.rs` and `.claude/rules/race-physics.md`.

## Typical parameter values (starting points — make these tunable, not hardcoded)

| Parameter | Typical range | Notes |
|---|---|---|
| `Crr` | 0.004–0.008 | Smooth asphalt; road bike tires |
| `CdA` | 0.30–0.40 m² | Hoods position |
| `CdA` | 0.25–0.30 m² | Drops |
| `CdA` | 0.19–0.23 m² | Aero/TT position |
| `ρ` | ≈1.225 kg/m³ | Sea level, 15°C; adjust for altitude/temperature if it matters later |
| `η` | 0.97–0.98 | Drivetrain efficiency |

Drafting reduces effective `CdA` for a rider sitting behind another rider/pack — a tunable
function of gap distance and pack position (design choice, not hard science). It enters the
physics as `aerodynamic_drag_multiplier` only; the model itself is `race_engine`'s
`BasicDraftingConfig` (see `docs/architecture/physics-design.md`, "Positioning and drafting").

## %FTP training zones (Coggan power zones)

Used by `session_director`'s `TerrainSynthesisStrategy` to map a workout interval's %FTP to a
plausible grade when there's no real GPS route to anchor to:

| Zone | %FTP | Typical name |
|---|---|---|
| Z1 | <55% | Active Recovery |
| Z2 | 56–75% | Endurance |
| Z3 | 76–90% | Tempo |
| Z4 | 91–105% | Threshold |
| Z5 | 106–120% | VO2max |
| Z6 | 121–150% | Anaerobic Capacity |
| Z7 | >150% | Neuromuscular |

`docs/architecture/physics-design.md` gives the grade-by-zone starting lookup
(Z2≈1–2%, Tempo≈3–4%, Threshold≈5–7%, VO2≈8–10%+), scaled by the rider's watts/kg — treat those as
a tunable starting table, not a derived physical law.
