# Product alignment

These decisions came out of an explicit product-alignment session (2026-09-21). Every slice-level
decision is made through them until a future session revises them. When they change, update this
document and the documents that depend on it in the same change. Flag unresolved conflicts in
`open-race-design-questions.md`.

## Product identity

A **Zwift alternative where races happen to be training-correct**. Both halves count equally;
neither is subordinate. The rider's goal is still to *match the workout*, and drama is the lever
that makes them want to. The emotions of a race push the rider to produce the prescribed effort:
getting dropped, bridging, winning the sprint. A race that does not deliver the workout is a game.
A workout delivered without a race is the spreadsheet this product exists to replace.

The drama levers, in priority order:

1. **Opponent believability**: bots feel like riders with intent, not pace-scripted NPCs.
2. **Outcome uncertainty**: every ride, the rider wonders "could I get dropped? could I win?"
3. **Narrative continuity and physical immersion**: deferred until the first two exist.

So the dominant design risk is not physics fidelity or the validation battery. It is **whether
opponents behave like riders with intent**. The believability budget goes to the few riders in the
drama spotlight before anything is spent on field size, visuals or statistical validation.

## First user and platform

- **v1 is built for the project owner as a dogfooder**, riding his own winter training on his own
  hardware. Generalizing to store users is not a v1 concern.
- **Ride device: a Linux desktop next to the bike, running the Godot app.** Other platforms follow
  [platform-rollout.md](platform-rollout.md).
- **Primary trainer: JetBlack Victory** (native FTMS). The real-hardware SIM-mode spike targets it
  explicitly; see "Hardware" below.

## Calendar

**No hard date.** Whatever slice is ready gets ridden each week. The ride screen, which rides the
synthesized route on a real or simulated trainer, keeps weekly riding alive while Slices 4–9 build
underneath it. The first full dogfooding target is auto-synthesized scenarios from imported `.zwo`
files (Slice 8). It is done when it is done: readiness drives each slice, not the calendar.

## Bot intelligence: hybrid, not full utility AI

- **The 2–3 drama-spotlight riders** run real utility-based tactical AI. These are the break
  instigator, the chase organizer and the rival at the finish. This is where the believability
  budget goes.
- **The pack runs scripted pace plans with seeded variance.** These are constrained,
  capability-honest templates that read as "a group with opinions" from the rider's point of view.
- **No rubber-banding in either tier.** Capability stays fixed, and scripts consume real physiology.
- **Escalate** more riders to utility AI on **evidence of visible breakage**, such as failed chases
  that look wrong or nonsense sprint timing, not on spec.

Spotlight AI needs something to have intent about. A minimal scene-goal seam therefore lands with
Slice 5. It is a scene goal expressed as a targetable future race-state set, which the spotlight
AI's utility can be evaluated against. Projections and fallback selection stay in the Scene Planner
work. Full pack/group recognition, cooperation and natural passing statistics arrive when they
visibly matter: larger fields, echelons, and the Slice 10 mechanics.

## One thin vertical, then generalize

Contracts, scenarios and the evaluator are built as **one thin vertical for one workout type**,
end to end. Generalizing to every type comes after. The mapping table in `physics-design.md`
eventually covers every workout type. The cheapest way to learn what a contract must contain is to
run one type through the whole loop and ride it.

- **The anchor type is threshold.** It maps to chase, bridge, paceline and long-climb situations.
  It is the bread-and-butter winter interval, and its power envelope is well defined.
- **Generalizing** to the remaining mappings follows once the first vertical survives a real ride.
- **Validation is light at first**: deterministic generation plus a cheap feasibility smoke-check,
  meaning the scenario is completable and the field is coherent. The heavy battery comes later,
  once it is shown to catch real failures. That battery forward-runs candidates across bounded
  under/at/over-target controllers and representative seeds, and reports reachability, fidelity
  and regeneration rate. Light validation is acceptable *because* a dogfooder rides each build.
  What is not cuttable is anchoring scenario generation to the workout itself: the race must be
  built around the contract, or the product thesis collapses.

## Visible development

The Slice 2 `RideSession` is a temporary integration path until the Slice 9 adapters land. During
development, the live ride needs an understandable visual. The Godot ride screen renders reported
power, speed and distance on the frozen route for real and simulated trainers. The simulated
trainer's internal controls remain debug-only.

Do not depict opponents, tactical actions or race outcomes as live until those states come from
the race kernel. The race screen meets that rule: its riders are stepped by the real kernel, and
it is labelled as running scripted powers.

## Hardware: the SIM-mode spike comes first

Real-hardware SIM-mode validation does not wait for Slice 9. With a JetBlack Victory in hand and a
Linux ride device, the **FTMS SIM-mode write-path spike runs in parallel with the simulation
work**. It writes gradient, wind and rolling-resistance parameters to the Victory and records:
- gradient caps;
- parameter units;
- resistance smoothing;
- latency.

These are the per-brand quirks CI cannot see. Finding them in Slice 9 would force adapter rework;
finding them now costs a one-off script. Findings go into [`docs/trainers/`](../trainers/) and
[risks.md](risks.md), and feed the Slice 9 adapter design directly.
