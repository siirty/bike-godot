# Race simulation, training & physics design

## Power → speed

Forward direction (`v` known → `P(v)`) is closed-form:

```
P(v) = [m_total·g·(sinθ + Crr·cosθ) + 0.5·ρ·CdA·(v+w)²]·v/η
```

Here `w` is positive headwind and negative tailwind, matching `EnvironmentParams::headwind_ms`.
The inverse (`P` known → solve `v`) needs a numeric solver because of the cubic aero term. Use a
safeguarded Newton–Raphson with bisection fallback, bracketed to `v ∈ [0, ~30 m/s]`. `P(v)` is
monotonic, so this converges in single-digit iterations. For the full formula, typical parameter
values and the %FTP zone table, see `.claude/skills/cycling-physics/SKILL.md`.

## Design invariant: who computes what

**Once a real trainer is in SIM mode, *the trainer's own firmware* already solves this physics
on-device.** It reports real speed and power back over FTMS. `.claude/skills/ftms-ble-protocol/SKILL.md`
explains why the app cannot do this itself: BLE latency and per-trainer resistance calibration both
rule it out. `race_physics`'s solver is for:

1. AI/peloton riders, who have no physical trainer and must be driven purely in software;
2. smoothing the human rider's position between the trainer's ~1–4 Hz BLE reports, for a smooth
   30–60 fps render;
3. pre-ride scenario feasibility checks, for example sizing a breakaway gap so that a reference
   rider following an eight-minute threshold contract catches it near the intended point;
4. terrain planning before any BLE session exists.

**Never re-simulate the human rider's trainer resistance independently of what the trainer itself
reports.** Doing so makes the resistance the rider feels diverge from what the screen shows.
The app owns *what* environment the rider feels: grade, wind, rolling resistance, and the
drafting-driven Cw. Once real hardware is connected, it does not own the trainer's sub-second
resistance actuation.

## Product invariant: the workout constrains the race, not the result

The app does not reproduce ERG mode indirectly. It turns the prescribed workout into a set of
physiological constraints. It then creates a race situation in which riding approximately the
prescription is a sensible way to achieve the objective.

The pre-ride pipeline is conceptually:

```
WorkoutPlan
    ↓
workout-shape recognition
    ↓
TrainingContract(s)
    ↓
SessionDirector
    ↓
RaceScenario
  ├─ terrain
  ├─ field composition
  ├─ AI physiology/personality
  ├─ race objectives
  ├─ scene goals / acceptable future race states
  └─ tactical opportunities
```

A `TrainingContract` is not a second-by-second watt trace. It describes the important stimulus:

- duration;
- target intensity, as a range or a ramp;
- allowable short excursions;
- recovery requirement;
- technique constraints, when present;
- eventually, metrics such as allowed work above CP and W′ cost.

The exact schema is still open; see `open-race-design-questions.md`.

The imported workout remains immutable and lossless. The intent contract and its evaluator are a
derived, versioned interpretation. That way, old generated sessions can be explained and
intentionally regenerated when derivation semantics change. Intent labels are metadata;
measurable envelopes are authoritative.

Recognition is hierarchical rather than interval-only. It preserves individual interval demands
and also the enclosing motifs, such as repeated work/recovery sets. It records whether each
grouping was explicit in the source or inferred. Examples include sustained, progressive,
regressive, over-under, hard-start, microburst, sprint, recovery, and mixed/race-like blocks.

The contract and the fidelity evaluator are one design artifact, used both before and after the
ride. Internally, fidelity stays multidimensional. The product exposes a versioned overall score
for comparison, together with transparent results for every interval and recognized block.

Initial scope is imported structured workouts. FTP tests are time trials. Generic unstructured or
free rides are not part of the initial product. A structured workout may still resolve to a
coached solo or group experience when a pack race is unsuitable.

## Deterministic race-state transition system

The race does not materialize every possible state as a graph node. On each simulation step, the
concrete `RaceState` is evaluated to determine the valid rider actions and their utilities or
probabilities. The deterministic kernel then applies the selected actions, physics and fatigue.
That produces the next state and typed domain events. Semantic labels such as peloton, breakaway
or chase are lower-dimensional projections, used by the director and the planner.

Conceptually:

    step(RaceState, RiderInputs, timestep)
      -> RaceStepResult(next_state, events)

Simulation time and seeded randomness are explicit. The kernel has no timer, trainer stream, Godot
or wall-clock dependency. The accelerated headless harness and the live adapter call the same
implementation.

The equilibrium power↔speed solver stays useful. Autonomous racing also needs dynamic
acceleration and inertia, coasting, braking and drafting. These mechanics are settled below,
before any AI behavior is judged. Drafting strength is the exception: its constants are explicit
but not yet calibrated.

### Accepted simulation contracts

These are settled, and they live in `race_engine` as pure Rust values. Later slices extend them;
they do not reshape them.

**Configuration versus state.** What a rider *is* and what a rider is *doing* are separate types.

- `RaceSimulationConfig` holds the fixed timestep, the frozen terrain, the ambient air density and
  headwind, the lateral-slot count, the dynamics configuration, the optional drafting model, and one
  `RiderDefinition` per rider. Each definition carries that rider's fixed `RiderPhysicalParams`.
- `RaceState` holds only what a step can change: simulation time, the RNG position, and one
  `RiderState` per rider. A `RiderState` holds distance, speed, lateral slot and current draft
  target.

This split is the mechanical form of no-rubber-banding: there is no field in which to change
capability mid-race. It is also why physiology (CP/W′, sprint capability) goes on the
*definition* side when Slice 5 adds it.

**Minimum rider state.** Distance, speed, lateral slot and draft target. The draft target is state,
not a derived value. A resumed race must not re-pick everyone's wheel from geometry and hand out
different shelter than the race that was interrupted.

**Lateral position is discrete.** The road is a small fixed number of slots, not a continuous
lateral coordinate. That makes occupancy, passing and being boxed in exactly and deterministically
resolvable, instead of an approximated continuous-collision problem.

**The controller boundary carries intent only.** One `RaceInputFrame` per step contains exactly one
`RiderControlInput` for every configured rider. Each input has:
- non-negative power;
- non-negative braking force;
- an optional requested lateral slot;
- an optional requested draft target.

Positioning fields are intentions the kernel resolves, never outcomes the controller asserts. A
controller that could write speed or position directly could rubber-band by construction. Frames
are total. A missing rider would turn "what does a rider with no input do?" into an unwritten
default.

Only software-simulated riders have an input variant. A human on a trainer does not produce a
requested power for the kernel to integrate; the trainer reports the power and speed that actually
happened. Reconciling that authoritative telemetry with simulated dynamics is Slice 9's problem,
and no variant is reserved for it in advance.

**Terrain is a value, not a callback.** `SimulationTerrain` is a serializable value with:
- distance-ordered `(distance, grade)` samples starting at 0 m;
- an explicit step or linear interpolation mode;
- an optional length;
- an explicit hold-last-grade or flat behaviour past the end.

A rider can always out-run a finite course, so the beyond-end behaviour is a required decision
rather than an edge case. Making terrain a value is also what makes "generated before the ride and
frozen during it" enforceable rather than aspirational.

**Randomness is `pcg32-v1`.** This is a repository-owned PCG-XSH-RR 64/32 generator, pinned by
published reference vectors. A library RNG whose state cannot be read and written would not do:
a snapshot could not resume the same sequence. `RandomState` carries the algorithm ID plus the
64-bit state and odd stream. Draws follow canonical rider order, so iteration order is part of the
result. Changing the algorithm or the seed→state mapping requires a new algorithm ID and snapshot
handling. It never silently redefines `pcg32-v1`.

**Canonical ordering.** `RiderId` is a validated non-empty string, ordered by ascending UTF-16 code
unit. That is the prototype's ordering, kept so snapshots stay byte-identical. Every rider
collection in these contracts is in that order. Building the same values from differently ordered
sources yields equal objects and identical encoded bytes.

**Validation is typed and atomic.** Illegal values are rejected at construction with a
`SimulationValidationCode` and a field path, and nothing partially accepted is observable.
Cross-object rules live on the configuration, which is the only thing that knows both halves.
These rules are:
- the rider set of the state or frame matches the configuration;
- slots are within the configured count;
- draft targets name another configured rider.

Contract fields are private, and every value is built through a validating constructor. So
"validated" is a property of the type, not of the entry point a caller happened to use. A change
is a rebuild through that constructor. For example, dropping a wheel is a new `RiderState` with no
draft target, which is an ordinary transition.

Rider collections come in two constructor forms:
- `new` canonicalises the collection;
- `canonical` adopts the list as given and rejects it if it is out of order.

So no value can exist whose encoded bytes depend on the order someone built a list in.

**Events are one exhaustive enum.** `RaceEvent` is a plain Rust enum, deliberately not
`#[non_exhaustive]`. Every consumer matches every variant, and the compiler proves no case was
missed. Adding a variant is meant to break those matches. Every variant carries the shared
simulation time and the canonically ordered involved riders, validated once in a shared check.
The current `Reference` variant is a placeholder so the seam can be exercised at all. The slice
that defines the real event vocabulary deletes it.

**A type's JSON belongs to the crate that owns the type.** `RiderPhysicalParams` derives its own
serde mapping in `race_physics`. The snapshot codec and the determinism-comparison helper both go
through that mapping rather than naming the fields again. A mapping written outside the owning
crate is a second enumeration that the compiler cannot keep in step. Here, the concrete cost would
be a physical parameter added later, silently dropped from a saved race, and restored as a default
after a restart. `workout_import` follows the same rule for segment JSON.

### Positioning and drafting

Each step resolves every rider's slot and wheel from one immutable pre-step snapshot, so
processing order can never create a physical advantage.

- **Slots.** Every in-range slot request is accepted. Occupancy is soft, and collision avoidance
  is Slice 5's question.
- **Working draft target.** The target is chosen in this order:
  1. the requested target, if eligible;
  2. the current target, if still eligible;
  3. the nearest eligible rider ahead, with ties going to the lower canonical ID.

  A target kept in the state is always still eligible under the post-step geometry.
- **Shelter.** `BasicDraftingConfig` takes a maximum gap and a minimum drag multiplier for each
  slot offset. For an eligible target at gap `g` and slot separation `s`, the multiplier is:

  `m = near[s] + (1 − near[s]) · g / max_gap`, clamped to `[near[s], 1]`.

  "Eligible" means `0 < g < max_gap` and `s < near.len()`. Otherwise the multiplier is exactly 1,
  so shelter decays linearly with gap and monotonically with lateral separation. The constants are
  explicit and uncalibrated, and there is no default model.

### Determinism guarantee

There are two different questions here, and they are deliberately answered differently.

- **Same build, same target: exact.** The same configuration, state and inputs produce exactly
  equal state and exactly equal events. Canonical snapshots are byte-identical. `PartialEq` and
  snapshot round trips compare every field exactly.
- **Across supported targets: exact for discrete values, tolerance-based for floats.** These
  compare exactly:
  - rider IDs;
  - enums;
  - durations;
  - collection order;
  - integer slots;
  - RNG state;
  - event types and order.

  A target that produces a different event or ordering has produced a different race. PCG32 is
  integer arithmetic, so it has no excuse to drift. Only doubles are compared, using
  `abs_diff <= abs_tol + rel_tol * max(|a|, |b|)`.

The tolerance lives in a **test** utility (`DeterministicComparison`, behind `race_engine`'s
`testing` feature), never inside a domain model. Rounding a rider's speed inside the model to make
a cross-platform test pass would change the simulation to flatter the test. The rounded value
would then feed the next step.

### Resumable snapshots

`encode_snapshot` writes a self-contained UTF-8 JSON document. It contains:
- the format identifier;
- `schemaVersion: 2`;
- the complete configuration, including terrain and rider definitions;
- the complete state, including the RNG position.

Decoding needs nothing else: no database, no original objects, no director rerun. Encoding uses a
documented canonical field order and canonical rider ordering. Durations are integer microseconds.
64-bit RNG values are fixed-width lowercase hex, because a JSON number would round the top of the
range and silently resume a different sequence.

No creation timestamp or other wall-clock value appears in the payload. One would break the
byte-identical encoding that the determinism tests rest on. A storage adapter that wants a
"saved at" column adds one beside the blob. Storage transport itself is outside `race_engine`.

`decode_snapshot` fully validates structure and domain rules before returning anything. Each of
these is a typed failure:
- an unknown format identifier;
- an unsupported schema version;
- malformed JSON;
- an unknown enum name;
- a violated invariant.

An unsupported version is rejected explicitly and never read as an older one. A snapshot is a
resume point with a lifetime of hours. It is **not** a permanent replay or history format, and
never a record of what the athlete produced.

### Longitudinal dynamics and the fixed timestep

The equilibrium solver answers what speed a power holds forever. `step_longitudinal_dynamics`
answers what happens in the next step, which is what acceleration, coasting, braking and a grade
change under a rider actually are. Both stay. The solver still sizes terrain and checks scenario
feasibility. A step taken at the solver's answer produces no meaningful acceleration.

**One fixed semi-implicit Euler step, and that is a contract.** The step works like this:
1. Forces are computed from the pre-step speed.
2. The next speed is `v + a·Δt`, clamped to `>= 0`.
3. Distance advances by the *clamped* speed over the full step.

There is no adaptive stepping and there are no hidden substeps. A race's trajectory has to depend
on its inputs, not on how hard the arithmetic was at a particular moment. That is the same
guarantee the fixed step in `RaceSimulationConfig` exists to give. A finer step is a measuring
stick, never a refinement.

**The rider never goes backwards.** The clamp is the model's one discontinuity, so it is reported
rather than hidden. A step that clamps is flagged, and the raw force-derived acceleration is
returned beside the applied result.
- A rider stopped by braking, or by a wall of a gradient, stays stopped under a net backward force.
- A descent or tailwind that assists strongly enough starts them moving with no rider power at all.

**Propulsive force is bounded at low speed.** `P/v` diverges at a standstill, so force is
`min(wheel_power / v, cap)`. Below the crossover speed `wheel_power / cap`, the cap applies; at
that speed the two branches meet exactly. The cap is an explicit, configurable *numerical*
parameter, not a gearing, torque or traction model; Slice 5's physiology replaces it rather than
tuning it. It travels in `LongitudinalDynamicsConfig` inside the simulation's configuration
rather than as a private constant. An input that does not appear in the configuration does not
appear in the snapshot either.

**Drafting enters as an aerodynamic multiplier and nothing else.** Shelter scales the aerodynamic
term. It never touches power, mass, rolling resistance, grade or speed. That restriction is what
keeps a drafting model from quietly becoming a rubber-banding lever.

**The step is 50 ms (20 Hz).** It was chosen by measurement, not by feel. Four candidates (200, 100,
50 and 20 ms) were run against a 2 ms reference of the same integrator, across a ten-scenario
corpus:
- acceleration from rest;
- a power step into coasting;
- braking with and without a stop;
- climbing;
- descending;
- a grade discontinuity;
- headwind and tailwind;
- a two-rider gap;
- an hour of steady riding.

The integrator converges at first order in every scenario. So the choice is a cost/accuracy trade,
not a stability threshold. 50 ms is where absolute trajectory error drops below the geometry the
race model has to resolve. At 100 ms, a rider is up to ~2 m from where a fine integration puts
them, which is the size of a drafting zone. `tests/convergence.rs` re-runs the corpus and holds
the budgets. To print the table, run:

`cargo run -p race_physics --features testing --example convergence_report --release`

**The resulting numerical budget is a downstream input, not a footnote.** At 50 ms the budget is:
- 0.10 m/s of speed;
- 1.0 m of absolute distance;
- **0.01 m in the gap between two riders on near-matching trajectories**;
- one step of stopping-time quantization;
- 0.5 m of stopping distance.

Drafting and position thresholds are designed against these numbers, not against an assumption of
exact continuous trajectories. The gap budget matters most: riders riding together share nearly
all of their integration error, while riders in different phases do not. `SimulationTimestep`
exports the budget as constants, so a threshold can name the number it depends on.

## Scenario is planned; race outcome is live

`session_director` is pure pre-ride planning. Its inputs are:
- the workout;
- an optional route;
- the rider profile;
- an explicit random seed, where variation is wanted.

From these it produces a deterministic `RaceScenario`. That scenario defines the terrain and the
major physiological story goals, but it **does not precompute AI position-over-time
trajectories**.

A story goal describes an acceptable set of future race states, not a single forced point. For an
eight-minute threshold block, for example, acceptable situations may include:

- chase a small break,
- bridge between groups,
- work in or hold an existing break,
- survive a climb selection, or
- sustain a hard paceline.

The preferred story may be "a 2–4 rider break is 10–30 seconds ahead when the threshold block
starts". Even so, the exact riders, the exact gap and the exact attack timing are soft outcomes. If
the human has already joined the break, the scene planner switches to a physiologically equivalent
goal, such as working in or defending the break. It does not force the original story back into
place.

This makes the planning problem receding-horizon rather than screenplay-like:

```
whole workout: SessionDirector chooses physiological scenes and terrain
minutes ahead: scene planner steers toward one reachable acceptable race state
seconds ahead: individual riders choose tactical actions
continuous: physics + fatigue determine what actually happens
```

The live Scene Planner, not `session_director`, is the only layer that replans after the race
diverges. It may coordinate bounded temporary roles, such as:
- instigator;
- follower;
- chase organizer;
- non-cooperator;
- protected rider.

It may also bias tactical utilities across several riders. It does not animate puppets: local
state, threat, physiology and physics can prevent the intended transition. It must not teleport
riders, rewrite physiology, or force an impossible action.

## No rubber-banding does not mean non-reactive AI

AI riders **may react tactically** to observable race state, including the human rider. A believable
opponent can follow an attack, decide a break is dangerous, refuse to pull, counterattack, move up
before a climb, or sprint for the finish.

The no-rubber-banding invariant is narrower and stronger:

> Race outcomes may react to the rider; rider capabilities may not.

Each AI rider has fixed physical and physiological capabilities for the session, such as mass, aero
parameters, the CP/FTP relationship, W′ and sprint capability. It also has stable tactical and
personality parameters. Those values are never secretly raised or lowered because the human is
winning, losing, over-riding or under-riding the workout. Tactical decisions consume the same
finite capabilities that would constrain any other rider.

A human riding easier may fail to bridge or get dropped. Riding harder may close a gap early and
reach shelter sooner. That is a legitimate race outcome, not something the engine repairs.

A persistent subset of named AI riders carries identity, physical and physiological capability,
tactical personality and reputation across sessions. Temporary fatigue, position and planner role
remain session state. The rest of the field may come from a broader variable roster. Reputation
helps decide whether a move is worth following, together with current-race threat. It is never an
omniscient replacement for race context.

The initial physiology model is CP, W′ balance and reconstitution, Pmax with short-duration decay,
mass, CdA and phenotype. Physiology and tactical personality are separate. Race-to-race form
variation is deferred.

## Constrained stochasticity

Riders come in two tiers:
- **The 2–3 drama-spotlight riders** (break instigator, chase organizer, the rival at the finish)
  run live utility tactics.
- **The rest of the pack** rides scripted pace plans with seeded variance. These plans are
  capability-honest templates that consume real physiology.

A rider is escalated to utility AI on evidence of visible breakage, not on spec. The full
utility-AI field is the end state. Everything in this section binds both tiers.

Individual rider AI evaluates locally valid actions from three things: observable race state, its
persistent profile, and its current physiology. It does not receive the human's target error,
fidelity score or future workout structure.

The live Scene Planner knows the current scene goal. It may assign bounded temporary roles or bias
tactical utilities across riders. It uses that influence to keep AI-driven transitions inside the
viable region for explicitly bounded reference athlete behaviors. Physics, fatigue and the threat
model still decide whether an action is plausible and whether it succeeds.

Conceptually:

```
concrete race state
        ↓
valid actions from physics + physiology
        ↓
local tactics + personality + threat   (spotlight)  /  pace plan + seeded variance (pack)
        ↓
bounded planner role/influence
        ↓
chosen actions → deterministic simulation step
```

For the defined reference cases, the app guarantees that AI behavior preserves an opportunity to
encounter the intended physiological problem. It cannot guarantee that arbitrary human behavior
remains naturally recoverable, or that the human produces the prescribed watts.

## Race primitives

Most structured workouts should be expressible by composing a small vocabulary of race situations
with terrain and drafting. The alternative, one bespoke story per workout file, does not scale.

- `PELOTON_SHELTER`: recovery / endurance.
- `PACELINE`: tempo / sweet spot.
- `WORK_BREAKAWAY`: sweet spot / threshold.
- `CHASE`: threshold.
- `BRIDGE`: threshold / VO₂.
- `ATTACK_RESPONSE`: VO₂ / anaerobic.
- `SPRINT`: sprint / neuromuscular work.
- `CLIMB_SELECTION`: tempo through VO₂, depending on gradient, duration and field strength.

Pack state uses a simplified 1.5-D model: distance along the road, a lateral slot, and a desired
wheel or draft target. It supports drafting, passing, positioning, group organization and being
boxed in, without full steering or collision simulation. Manual steering is not part of the initial
product. High-level athlete positioning controls remain an optional later addition.

Draft position and pull duty are first-class control inputs. An over-under, for example, can be
represented naturally by rotating between two positions instead of manufacturing a perfectly
periodic hill:
- shelter, for the "under";
- the front of a break or paceline, for the "over".

Terrain still matters, but it is only one lever. The others are group speed, drafting position,
field composition, AI intent and existing gaps.

## Workout translation rules and examples

The director should translate the *whole workout* into a coherent race arc. It should not paint
each interval independently onto unrelated terrain.

Common mappings:

- recovery → sheltered bunch, descent, or regroup;
- endurance → steady peloton or group ride with benign positional movement;
- tempo → fast rotating paceline or controlled chase;
- sweet spot → working breakaway or sustained chase;
- threshold → chase, bridge, hold a break, or long climb selection;
- over-under → rotate between pull duty and draft;
- hard-start threshold → attack to establish a break, then sustain it;
- VO₂ → bridge to a group, cover attacks, or short-climb selections;
- microbursts / 30-30s → attacks and counters over rolling or criterium-like terrain;
- anaerobic → explosive attack, gap closing, or a short ramp;
- sprint → intermediate or finish sprint;
- technique-defined work, such as low or high cadence → race or terrain context plus an explicit
  cadence cue, because terrain alone cannot enforce a gearing choice;
- FTP/threshold test → time trial, where tactical interaction should not contaminate the test.

Progressive and regressive blocks should also come from several mechanisms, not gradient alone.

A progressive tempo→threshold block can arise from any of:
- a chase group losing riders;
- longer pull duty;
- diminishing draft;
- a rising road.

A regressive block can arise from:
- a break becoming established;
- cooperation improving;
- the rider spending longer sheltered;
- the terrain easing.

## Scenario feasibility and fidelity

Scenario generation is anchored to the training contract: the race is built around the workout.
That is not cuttable, because without it the product thesis collapses.

Validation starts **light**: deterministic generation plus a feasibility smoke-check, meaning the
scenario is completable and the field is coherent. The dogfooder rides every build, and that is
part of the validation loop.

The heavier machinery is added later, once it is shown to catch real failures. Candidate scenarios
are simulated with explicitly bounded under-target, at-target and over-target reference
controllers. The director searches over variables until the intended stimulus and outcome
sensitivity fall inside acceptable ranges. Those variables include:
- starting gap;
- AI strength;
- group size;
- gradient;
- drafting.

The guarantee is bounded. AI-driven transitions must not eliminate every training-equivalent
continuation for those reference behaviors. Arbitrary human behavior is never restricted, and it
may leave no natural viable path. In that case, the live Scene Planner first seeks an equivalent
fallback. A visible neutralization or regroup is the current preference for the exceptional
fallback, but it is still unresolved.

Example: for an eight-minute threshold chase, a reference rider near target power might be expected
to close the gap around minute 7–8. Under-riding should often fail, and over-riding should close it
earlier. The exact tolerances and fidelity metric are open design questions.

The generated race should not be expected to reproduce the smooth ERG trace second by second. Short
tactical excursions and sheltered periods are desirable, as long as these remain appropriate:
- interval-level load;
- time in the intended intensity range;
- work above CP;
- recovery characteristics.

The deterministic headless simulator is built first. It is the executable foundation for physics,
AI, planner and scenario validation. Candidate scenarios are tuned statistically across many random
seeds, using the same stepping kernel that the live app drives.

## Failure, objectives and finish handling

Race result, tactical objectives and training fidelity remain separate. Winning does not compensate
for poor training execution, and high fidelity does not imply winning. The field keeps attacking,
cooperating and chasing when the athlete rides above or below the prescription. It never carries
the athlete through or yields a result.

Ordinary player failure permanently changes the race state. There is no rewind or position
restoration. The manager communicates the race consequence and the revised objective. The Scene
Planner replans from the new state, while the remaining `TrainingContract` stays unchanged.
Intensity reduction is an explicit athlete action, never silent adaptation.

## Sprint handling

A race narrative may naturally continue into a finish after the prescribed race-driving work. When
that remaining effort is not part of the workout, the athlete explicitly chooses between two
options:
- keep racing physically;
- begin the cooldown, while the engine simulates the unresolved finish from the current race state
  and remaining capabilities.

If the workout itself prescribes a compatible finishing effort, such as a sprint, it remains hard
contract work. It cannot be replaced by simulated athlete performance. A scenario may also end
without a sprint, when its objective already resolves the race.

Simulated gameplay must never create fake power samples in the recorded training activity. Training
files contain only what the human actually produced. Simulated race results are separate game
state.

## Player information

Manager radio messages explain the tactical requirement. The HUD can show power, heart rate,
cadence, gradient and terrain, and the expected demand envelope. Guidance is layered and
configurable. In particular, the athlete may hide manager messages and/or the target envelope, to
practice reading the race without explicit instruction.

## Reproducibility and interruptions

Persist enough context to reproduce generation and explain behavior:
- the workout reference;
- the derivation and evaluator version;
- the generated scenario and terrain;
- the random seed;
- the relevant persistent rider-profile versions;
- important actions and transitions.

Bit-exact live telemetry replay is not an initial requirement.

When technical input loss is detected, race simulation pauses after a configurable grace period,
so a hardware failure does not become a race result. Voluntary zero power with healthy input
remains a normal race action, with normal consequences.

The simulation remains pure Rust, with no Godot dependency. Establish a benchmarked ceiling for
fully simulating every rider before considering a reduced-fidelity population for distant riders.
The athlete's local group and the tactically relevant riders always keep full fidelity.

## Terrain strategies

- **`TerrainSynthesisStrategy`**: generate the complete course before the ride and freeze it during
  simulation.
  - The Workout Director decides, across the hierarchical workout, which demands should arise
    mainly from terrain and which from race mechanics.
  - A threshold block may use a climb, but threshold never implies a climb by default.
  - The current `GradeForPowerSynthesis` (effort → grade) is only a seed heuristic.
  - Terrain respects plausible gradients, climb lengths, transitions and total elevation. It must
    also stay compatible with viable race paths.
- **`TerrainAlignmentStrategy`**: when a real GPS route is provided, the terrain is fixed. Workout
  goals are matched and anchored onto suitable real segments. An example is an actual climb long
  and steep enough to support a threshold chase for this rider.

## Open design work

This document is the source of truth for accepted race behavior. The remaining
implementation-gating schemas and empirical tuning questions are tracked by slice in
[`open-race-design-questions.md`](open-race-design-questions.md). Resolve only the questions needed
for the active slice, promote durable answers back here, and keep numerical tuning configurable.
