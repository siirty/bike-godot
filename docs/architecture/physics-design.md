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

## Product invariant: train the tactical eye; simulate credible consequences

[ADR 0007](../decisions/0007-tactical-training-core.md) and
[TT-01–TT-10](../requirements/tactical-training.md) define the accepted product behavior. The
unit of design is a tactical learning situation, not reproduction of a full race. Workout effort,
recognition/timing, continuity and result plausibility have separate acceptance criteria.

The workout constrains the opportunity, not the result.

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
  ├─ player objectives, role and available knowledge
  ├─ connected tactical situations / acceptable response and exit windows
  └─ bounded opponent action/role plans
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
or chase are lower-dimensional observations used to compare planned situations with what actually
happens. An authored scene name is not an observation or a tactical-validity oracle.

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

**Events are a report derived from two states.** A step's events are computed only from the
pre-step state, the post-step state and the configuration. They are never an input, never part
of `RaceState`, and never in a snapshot. So a race resumed from a snapshot reports exactly the
events an uninterrupted one would have.

A `RaceEvent` is the post-step simulation time plus a `RaceEventKind`. The kinds are deliberately
limited to what the kernel can observe today:

- **`SlotChanged { rider, from_slot, to_slot }`**: the resolved slot differs.
- **`WheelLost { rider, target }`**: the rider had `target` and now has none, or has another.
- **`WheelTaken { rider, target }`**: the rider now drafts `target` and did not before.
- **`Overtook { rider, passed }`**: `rider` was strictly behind `passed` before the step and is
  strictly ahead after it. An exact tie is neither ahead nor behind, so passing through one is not
  reported.
- **`Finished { rider, place }`**: the rider reached the terrain's explicit length this step. A
  course without a length has no finish. Places count riders already past the line before the
  step. Same-step finishers are ordered by when they crossed inside the step (linear in the step's
  distance), then canonically. A rider that starts past the line is counted, but never reported.

The emission order within a step is part of the contract. It goes by kind in declaration order.
Finishers then go by place; every other kind goes by its primary rider and then the other rider,
canonically. So the order never depends on the order detection ran in.

`RaceEventKind` is deliberately not `#[non_exhaustive]`. Consumers match every kind, and a new
kind is meant to break those matches. Group events (split, catch, dropped, bridge) need group
detection, which is Slice 5's. They join the enum when that lands.

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
- **Shelter.** `BasicDraftingConfig` carries minimum and maximum center-to-center gaps and
  minimum drag multipliers by slot offset. Eligibility requires `min_gap < g < max_gap` and
  `s < near.len()`. Overlapping and side-by-side riders get no shelter in any slot.

  `m = near[s] + (1 − near[s]) · (g − min_gap) / (max_gap − min_gap)`.

  The result is clamped to `[near[s], 1]`. `new` uses a provisional 2.5 m minimum: a 1.8 m bicycle
  plus 0.7 m clear air behind its rear wheel. `with_min_gap` makes the geometry explicit; zero
  reproduces historical reference fixtures. Gap and multiplier values require calibration.

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
- `schemaVersion: 3`;
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
parameter, not a gearing, torque or traction model; a future drivetrain model replaces it rather than
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

## Director-first situation planning; outcomes stay live

`session_director` remains pure pre-ride planning. Inputs include the immutable workout, optional
route, rider profile, scenario assumptions and explicit seed. It produces a deterministic plan of
terrain, field, player objectives/knowledge and connected learning situations. Each situation
specifies cues, response windows, acceptable actions, physiological demand, consequences and exit
conditions as required by TT-01. Exact schemas and scheduling anchors remain unresolved.

The initial design compiles bounded opponent intentions or tactical roles before the ride. It may
use offline nominal simulations to size effort and gaps, but must validate with independent
execution policies and variations. It does not impose position/speed trajectories or a winner.
Terrain and capability are frozen; ordinary kernel stepping resolves all actual movement, fatigue,
drafting and finishes. Opponents may still respond locally to observable race state.

Validate that reasonable accepted execution leaves the remaining workout situations available in
sequence. Alternative wheels, gaps and formations are allowed when they preserve the learning
problem and effort envelope. For example, a threshold opportunity may be a bridge or work in an
existing break if the authored plan allows both and the cues remain credible. A label change alone
does not demonstrate equivalent effort.

A general live Scene Planner is not an initial prerequisite. There is no automatic target-error
repair, neutralization or regroup to restore missed opportunities after ordinary rider mistakes.
Record failures and actual consequences. Technical-disconnect pause handling remains separate.
The need for any future bounded adaptation must be demonstrated and accepted explicitly.

The existing `TacticalRole` seam can carry bounded intentions; its presence does not mandate a
runtime replanner. [ADR 0006](../decisions/0006-scripted-race-prototype.md) records the stricter
experimental frozen-command policy. Production may use local reactive controllers without
changing the pre-ride ownership boundary.

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
tactical personality and reputation across sessions. Temporary fatigue, position and assigned tactical role
remain session state. The rest of the field may come from a broader variable roster. Reputation
helps decide whether a move is worth following, together with current-race threat. It is never an
omniscient replacement for race context.

The physiology model is `race_physics::PhysiologyParams`: independent CP (W), W′ capacity (J),
and fresh peak power Pmax (W). `RiderDefinition` carries capability and `RiderState` carries
remaining W′. Config validation requires matching presence and a balance within capacity.
The corpus and live demo use explicit provisional capability fixtures; no universal CP/FTP
conversion is assumed. Physiological phenotype and tactical personality distributions remain open.

For requested crank power P below CP, the Skiba differential recovery equation is
`dB/dt = (W′ − B) (CP − P) / W′`. The exact constant-power update is
`Bnext = W′ − (W′ − B) exp(−(CP − P) dt / W′)`. At CP the balance holds; above CP, actual
excess work drains it one joule per joule. The reference is
[Skiba et al. (2015)](https://doi.org/10.1007/s00421-014-3050-3).

The simulation adds a reserve-dependent instantaneous ceiling
`Pavailable = CP + (Pmax − CP) B/W′`. This is a provisional sprint approximation, not a
neuromuscular-fatigue claim from Skiba. On an all-out request it yields exponential decay with
`tau = W′/(Pmax − CP)`. `step_physiology` integrates both the linear drain and any subsequent
ceiling-bound portion analytically, returning the actual average power delivered across the step.
That power drives the longitudinal dynamics. W′ never goes negative, and an empty rider can
still deliver CP. Rest below CP replenishes reserve and restores peak capacity.

`PackPacePlan` generates a repeating ten-minute schedule from an explicit PCG32 seed, using
30-second blocks at 85–100% CP with occasional 130–160% surges. When reserve falls below 20%,
the request blends continuously toward 65% CP as reserve falls to zero. Plans read only their own
rider's capability, reserve and simulation time. They are immutable scenario input policies;
resuming a kernel snapshot requires the same future inputs, as it always has. Capability limiting
lives in the kernel, so a controller cannot bypass it with an excessive request.

Schema v3 serializes physiology, reserve and minimum drafting gap. Schema v2 is rejected by the
production decoder. Frozen historical races remain regression evidence via test-only v3 fixtures
that explicitly select null physiology and the original zero minimum gap. No old bytes are
silently upgraded in production. Race-to-race form variation is deferred.

## Spotlight controller (Slice 5)

`race_engine::tactics::TacticalController` now supplies recover, follow, chase, cruise, attack and
sprint intentions for the two spotlight opponents. It observes one immutable pre-step state and
its own reserve, with seeded fixed personality. The pack retains `PackPacePlan`. Defaults are
provisional: 1 s decisions, 3 s minimum commitment, 0.10 switching margin and 20%/40% recovery
hysteresis. All tuning is validated and checkpointed. Following corrects gap/speed with a bounded
power request; the kernel alone applies physiological limits and physical movement.

A `TacticalRole` adds at most 0.15 utility to an already valid action. Recovery overrides roles.
This supplies a bounded role-input seam; it does not implement the situation compiler, player
knowledge model, continuity validation or a general live Scene Planner. Controller checkpoints
must accompany a kernel snapshot for exact autonomous continuation. See [ADR 0005](../decisions/0005-spotlight-tactics.md).

## Constrained variation and local tactics

Spotlight utility controllers and seeded pack pace plans are implemented policies, not a mandate
for full-field utility AI. Use the simplest policy that makes the tactical situation and its
consequences credible. Stochasticity uses explicit seeded RNG and remains reproducible.

Local reactive AI evaluates valid actions from observable race state, its fixed profile, current
physiology and bounded assigned intentions/roles. It receives no human target error, fidelity,
future workout or hidden lesson answer. The director may use workout knowledge offline to plan
and calibrate those intentions; it cannot use live target tracking to keep the athlete attached.
Capability, physiology, threat and physics still bound execution.

For declared bounded reference behaviors, generation checks sequential access to the remaining
training opportunities. This is a pre-ride validation obligation, not a runtime guarantee for
arbitrary input. It must not be implemented by rewinding, teleporting, secretly changing power or
forcing an impossible action.

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

The director translates the *whole workout* into connected tactical learning situations. Local
lessons must fit a coherent course, field and progression; this does not require reproducing a
complete real-world race. Do not paint each interval onto unrelated terrain or reset riders between
blocks. Validate the resulting path to contention as well as each opportunity.

Common mappings:

- recovery → sheltered group, descent, or a preplanned physically plausible regroup;
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

## Situation validation, fidelity and result plausibility

[TT-01–TT-10](../requirements/tactical-training.md) specify the independent acceptance dimensions:

- **Workout fidelity**: action execution offers the prescribed effort, duration and recovery.
- **Tactical learning**: players can classify the move and its timing from information available
  to them; coaching and target hints can be hidden without removing essential context.
- **Continuity**: accepted bounded execution enables later steps without resets or live rescue.
- **Race plausibility**: course, capability, selections, cooperation, gaps and accumulated fatigue
  explain the local consequences and any finishing result.

The first vertical needs focused evidence for all four. A prescribed-watt replay is a control, not
proof that the race induces that effort. Add a cue/action-following policy independent of workout
watts, continuous bounded variation and isolated mistakes. Mechanical pace-demand estimates are
supporting diagnostics, not proof of tactical necessity. Human observation is needed for cue
readability; a software controller alone does not demonstrate learning transfer.

Use matched scenes with changed team/strength/objective/course context and equivalent scenes with
changed identities. Check whether players use the relevant information rather than memorize an
avatar or phase label. Report unavailable information and uncertain interpretations explicitly.

Background moves must be legibly safe to ignore under stated conditions. Covering one may improve
position or change a later placing; that alone does not make the move mandatory. Validate whether
ignoring creates an unplanned race-saving obligation relative to the player's objective.

Candidate generation may tune gaps, field composition, terrain and policy parameters offline;
freeze the resulting capabilities and course before execution. Numerical effort bands, response
windows, acceptable formations and proficiency distributions need calibration. The prototype's
13 geometric phase gates are not production acceptance criteria. Riding ahead or taking a different
wheel is not automatically failure. After finishing, cooldown requires recovery, not race formation.

A sprinter's win on a course with 4,000+ m ascent requires inspecting the path to contention:
climb placement, grade, duration, selection pace, regroup opportunities and remaining reserve.
Total ascent and archetype alone cannot prohibit or justify the outcome. Explain surprising
results from simulated events; never manufacture the expected winner. Proficiency assumptions
must be explicit and distinguish physical capability from anticipation, positioning and cooperation.

## Failure, objectives and finish handling

Workout fidelity, tactical performance and race result are separate. Winning cannot compensate
for missing training; target watts cannot validate an unreadable lesson. Ordinary athlete mistakes
permanently change state and may lose later opportunities. Report the consequence and which
remaining opportunities were actually reached; do not restore the nominal plan automatically.
Intensity reduction remains an explicit athlete action. The workout and training history are not
silently rewritten to match what happened.

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

## Player information and tactical readability

Player knowledge is part of the authored situation, not merely a HUD preference. Provide objective
and role, recognizable relevant riders, team affiliations, qualitative known strengths, relevant
terrain/finish context and observable recent actions. If a required judgment depends on a fact,
communicate it beforehand or make it observable. Unknowns are allowed when explicit and fair.
Use a roster/briefing or equivalent accessible presentation; exact UI and persistence remain open.

Distinguish stable knowledge from inferred current danger and private AI state. Never expose exact
opponent reserve or hidden intention as if a real rider knew it. Reputation does not override the
current constellation. A permanent dangerous/safe avatar label does not teach move classification.

Optional manager radio and target-envelope coaching can explain decisions during guided practice.
Both can be hidden for assessment while essential context remains available. Physical movement,
constellation, timing and terrain must carry the lesson without a flashing threat indicator.
Post-situation feedback should explain relevant cues and consequences, separating recognition,
timing and effort. HUD power, heart rate, cadence and route information remain configurable.

## Reproducibility and interruptions

Persist enough context to reproduce generation and explain behavior:
- the workout reference;
- the derivation and evaluator version;
- the generated scenario and terrain;
- the random seed;
- the relevant rider-profile versions and proficiency/model assumptions;
- player-visible briefing, affiliations, known strengths, objective and coaching settings;
- planned situations/cues/windows separately from observed actions and transitions;
- effort/reserve, gaps and selections supporting the path to any result.

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

## All-zone fixed-action experiment and implementation status

[ADR 0006](../decisions/0006-scripted-race-prototype.md) scopes `tools/race-prototype`: a synthetic
Z1–Z7 fixture over the existing kernel, with intentions frozen after offline fitting and scheduled
versus observed transitions reported separately. ADR 0007 adopts director-first situation planning
as the initial direction; it does not promote this tool to a validated production compiler.

The prototype lacks the player knowledge model, human recognition validation and a progression/
result plausibility evaluator. Its provisional formation gates can reject acceptable alternatives.
Neither nominal fitting nor sensitivity to deviations settles real-world lesson validity. The
existing reactive tactics, physiology and kernel remain unchanged; production requirements added
here still require implementation and evidence.

## Open design work

This document is the source of truth for accepted race behavior. The remaining
implementation-gating schemas and empirical tuning questions are tracked by slice in
[`open-race-design-questions.md`](open-race-design-questions.md). Resolve only the questions needed
for the active slice, promote durable answers back here, and keep numerical tuning configurable.
