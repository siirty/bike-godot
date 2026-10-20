---
description: Pre-ride planning versus live simulation, and the deterministic kernel contracts
paths:
  - "crates/session_director/**"
  - "crates/race_engine/**"
---

# session_director / race_engine boundary

This split is what carries the "workout constrains the race, not the result" principle. Read
`docs/architecture/physics-design.md` and `docs/architecture/open-race-design-questions.md`
before changing either crate.

- **`session_director` is pure pre-ride planning.** Its inputs are:
  - a `WorkoutPlan`;
  - a `RiderProfile`;
  - an explicit seed, where variation is wanted.

  From these it produces workout-derived `TrainingContract`s and a `RaceScenario`, which covers:
  - terrain;
  - field composition;
  - fixed AI capabilities and personalities;
  - race objectives;
  - major physiological scene goals, as acceptable future race states.

  It has no wall clock, no live rider state and no I/O. It must stay deterministic for a fixed
  input and seed.
- **`session_director` does not precompute complete AI position-over-time trajectories.** It
  scripts the opportunity, not the exact outcome.
- **`race_engine` owns the race simulation and live replanning.** Its core is a pure deterministic
  stepping kernel with explicit time, seeded randomness, rider inputs and typed events.
  - The headless harness calls that kernel directly.
  - The live adapter supplies trainer telemetry, drives stepping from Godot's frame loop, presents
    events and state, and writes SIM parameters.
  - Neither timers nor trainer streams belong inside the kernel.
- **The short-horizon Scene Planner is the only live layer that replans** after player or AI
  actions make the preferred scene unreachable. `session_director` remains pre-ride and never
  consumes live race state.
- **AI riders may react to the human and to each other.** None of these is rubber-banding:
  - following an attack;
  - refusing a pull;
  - chasing a dangerous break;
  - counterattacking;
  - changing position before a climb.
- **Rubber-banding means changing capability or physics to force an outcome.** Never secretly
  change any of these because the human is ahead or behind, or over or under the workout:
  - AI CP/FTP;
  - W′;
  - sprint capability;
  - mass or aero values;
  - hidden speed or power boosts.

  Never teleport riders, or force impossible actions to satisfy a story beat.
- **The pack tier's scripted pace plans are bound by the same rule.** A pace plan chooses
  intentions within the rider's fixed physiology. It never reads the human's workout target or
  error.
- **The scene planner may assign bounded temporary tactical roles** and bias intentions and
  transition utilities, but only within each AI rider's available physiology and plausible threat
  model. Individual rider AI observes local race state, not the human's target error or fidelity
  score.
- **Viability guarantees are evaluated for explicitly bounded reference rider behaviors.**
  AI-driven transitions must not eliminate every training-equivalent continuation for those cases.
  Arbitrary human input stays authoritative. It may require a live fallback or, exceptionally, a
  separately defined neutralization.
- **The app presents the intended physiological problem; it does not enforce the human's watts.**
  A rider who under-rides may get dropped or fail to bridge. A rider who over-rides may catch early
  and gain shelter. Do not add hidden correction logic that recreates ERG mode.
- **Simulated gameplay outcomes never create synthetic power samples** in the recorded training
  activity. An athlete-selected simulated finish is one example of such an outcome.

## Deterministic simulation contracts (`race_engine/src/simulation/`)

These are documented in `docs/architecture/physics-design.md` ("Accepted simulation contracts").
They are invariants to protect, not preferences.

- **Capability lives in `RaceSimulationConfig`; only dynamic data lives in `RaceState`.** Never add
  a capability field to `RiderState`, and never have a step rewrite the configuration. Physiology
  (CP/W′, sprint) belongs on `RiderDefinition` when Slice 5 adds it.
- **No library RNG anywhere in the kernel** (`rand`, `thread_rng`, `RandomState` hashing order, …).
  Randomness is `Pcg32` (`pcg32-v1`), whose full state is in `RaceState` so that a snapshot resumes
  the same sequence. Changing the algorithm or the seed→state mapping needs a **new algorithm ID**
  and snapshot handling; never redefine `pcg32-v1`. Draws follow canonical `RiderId` order. Never
  iterate a `HashMap` where the order can reach the result.
- **Do not add a human or externally-observed `RiderControlInput` variant.** Slice 9 owns
  reconciling trainer-authoritative telemetry with simulated dynamics. Reserving a shape now would
  decide that question early.
- **Positioning inputs are intentions.** A controller requests a lateral slot or a wheel, and the
  kernel resolves it. Never let a controller write speed, position or slot directly. That is
  rubber-banding by construction.
- **Tolerance never enters a domain model.** `PartialEq` and same-target snapshot round trips are
  exact. Cross-platform float comparison lives in `DeterministicComparison`, behind the `testing`
  feature. Do not add rounding inside `RiderState`/`RaceState` to make a test pass.
- **Snapshots stay self-contained and versioned.**
  - No wall-clock value goes in the payload.
  - No reference to a file, database or director run.
  - An unsupported `schemaVersion` is rejected, never read as an older one.

  Storage transport belongs outside `race_engine`; the codec only produces bytes. A schema change
  bumps `SNAPSHOT_SCHEMA_VERSION`. The previous version's golden snapshots stay as rejection
  tests.
- **Validation is atomic and typed.** Contract fields are private, and every value is built through
  a constructor that validates with a `SimulationValidationCode` and a field path. That makes an
  illegal value unrepresentable rather than merely discouraged. Do not add a `pub` field or an
  unchecked constructor to a contract type. Rider collections keep the split: `new` canonicalises,
  and `canonical` adopts the list and rejects a non-canonical one (`RidersNotCanonical`).
- **`RaceEvent` is one exhaustive enum.** Do not mark it `#[non_exhaustive]`: consumers matching
  every variant is the point. The shared time/rider validation runs for every variant. `Reference`
  is a placeholder for the seam, and the slice that adds the real vocabulary deletes it.
- **A type's JSON belongs to the crate that owns the type.** The snapshot codec must not enumerate
  the fields of a model owned elsewhere. It goes through that crate's own serde mapping, as it does
  for `RiderPhysicalParams`. Otherwise a field added there is silently dropped from saved races.
- **Parity:**
  - `tests/golden.rs` replays the Dart prototype's recorded races and snapshots bit for bit;
  - `tests/kernel_properties.rs` holds the invariants.

  A deliberate change to existing behaviour goes in `docs/port/divergences.md` and is named in the
  golden test. New behaviour has no Dart oracle. Derive its tests from the spec, published
  references or independent calculation, never from the implementation's current output.
