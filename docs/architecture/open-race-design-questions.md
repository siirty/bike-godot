# Open race-design questions

This file is the unresolved design and tuning backlog for the headless-first race system. Accepted
architecture does not live here:

- physics, simulation, training and race behavior: [physics-design.md](physics-design.md)
- crate ownership and dependencies: [README.md](README.md)
- implementation sequence and verification: [build-order.md](build-order.md)
- product identity and priorities: [product-alignment.md](product-alignment.md)
- pre-ride versus live ownership rules:
  [session-director-race-engine.md](../../.claude/rules/session-director-race-engine.md)

Do not copy resolved decisions back into this file. When an item below is decided, promote the
durable rule to its authoritative document and remove the item here. Numerical constants and tuning
values should stay configurable until headless evidence supports them.

## Slice 5: headless autonomous race

Live utility tactics are for the 2–3 drama-spotlight riders only, plus a minimal scene-goal seam.
The pack runs scripted pace plans with seeded variance. The first autonomous simulation uses
hand-authored terrain and starting state. It needs empirical answers for:

- athlete-specific CP/FTP estimation and calibration of W′ capacity;
- empirical calibration of Pmax and the reserve-dependent sprint ceiling;
- initial rider-profile and tactical-personality distributions;
- the spotlight riders' utility functions, action set, decision cadence, reaction delay and
  commitment/hysteresis;
- the threat model, combining persistent reputation with current-race danger;
- richer pack pace templates and recovery behavior, guided by the seeded baseline;
- the shape of the minimal scene-goal seam, meaning a targetable future race-state set that the
  spotlight AI's utility is evaluated against;
- collision avoidance under soft slot occupancy;
- calibrated bike-length, wheel-clearance and shelter constants by gap, depth and lateral slot;
- group events (split, catch, dropped, bridge) and their detection rule, to extend `RaceEventKind`;
- the initial full-fidelity field ceiling and benchmark hardware;
- the seed corpus, stability duration and statistical acceptance criteria for calling the
  simulation natural and numerically stable.

Named riders use persistent-shaped fixtures in this slice. Database persistence, progression and
leaderboard updates are not prerequisites for simulation behavior.

## Slice 6: Training Contract and evaluator

These semantics are accepted:
- versioned intent envelopes, derived from immutable workout data;
- one shared evaluator;
- both an overall score and interval/block explanations.

The first cut covers threshold only. Implementation still requires decisions about:

- the minimum contract fields for steady, ramp, recovery and repeated structures;
- deterministic hierarchical shape-recognition rules and adjacent-step merging;
- how explicit source repetition differs from inferred grouping;
- acceptable intensity bands, minimum useful time, excursion magnitude/duration and duration
  tolerance by workout shape;
- ramp fidelity, recovery quality and above-CP/W′ semantics;
- whether technique-defined steps extend the same contract or use a parallel coached-step model;
- the exact under/at/over-target bounded reference controllers used by feasibility checks;
- which evaluator dimensions gate validity and which are only reported;
- mapping the multidimensional result to a versioned 0–100 score;
- how to treat structured workouts that contain free-ride segments. Whole-session unstructured or
  free rides remain outside the initial scope.

## Slice 7: Scene Planner

The live Scene Planner owns short-horizon steering and replanning. Its questions are answered when
the threshold vertical reaches the planner layer. They are:

- the semantic projection from concrete race state into situations such as peloton, breakaway,
  chase, bridge and selection;
- the `SceneGoal` and planner input/output types;
- planner cadence, horizon, and how early it may prepare the next workout block;
- the exact temporary tactical roles and the typed, bounded influence channel;
- how strongly planner influence may bend personality without erasing a named rider's character;
- the finite-horizon viability representation for bounded reference behavior;
- which AI transitions are blocked when they would remove every training-equivalent continuation;
- fallback ranking, continuity cost, replan triggers and hysteresis;
- the exceptional live fallback when arbitrary human behavior leaves no natural viable path.
  Visible neutralization/regroup is the current preference, but it is not yet accepted.

## Slice 8: Workout Director and scenario synthesis

The pre-ride director owns the whole-session outline, the generated frozen terrain, the field and
the scenario. Generation is deterministic, with a light feasibility smoke-check first. It still
needs:

- the `RaceScenario` schema, and the generation order between outline, terrain, field and scene
  goals;
- the initial reusable situation/template vocabulary, and each template's entry, demand and exit
  contract;
- the low-dimensional free parameters per template, and which of them support monotone solving;
- distributions for terrain gradient, climb length, smoothing and total elevation;
- how the director balances terrain-driven demand against race-mechanics-driven demand;
- field composition, persistent named-rider selection, and generation of variable supporting
  riders;
- feasibility outcome windows, sensitivity targets and the generation-time compute budget;
- once the heavy validation returns, how many seeds are needed during generation versus offline
  Monte Carlo validation;
- regeneration limits, and the structured non-race fallbacks for an infeasible pack race;
- which coached solo/group experiences are supported initially, beyond the explicitly supported
  FTP-test time trial.

## Slice 9: human, trainer and UI integration

The simulation is integrated with the athlete only after headless validation. Remaining questions
include:

- how athlete CP, W′ and Pmax are initialized from available profile/test data, and how bad
  calibration is detected;
- the exact technical-disconnect grace period, and the pause and resume/checkpoint behavior.
  This includes how workout time and wall time sit beside simulation time, which is settled as a
  monotonic count of fixed steps in `RaceState`, and which of those clocks pause;
- reconciling trainer-authoritative speed with the simulation's dynamic physics, drafting and pack
  state;
- the timing and default presentation of the explicit ridden-versus-simulated finish choice;
- the separate data types and storage paths that keep simulated performance out of measured history
  and training load;
- manager-radio event selection, wording and delivery cadence;
- the default HUD layers, and how finely to configure radio messages, target envelope, power, heart
  rate, cadence and terrain;
- post-block feedback, and how overall and interval fidelity are presented;
- the persistence schema for scenario seed, generated terrain, rider-profile versions and important
  events/transitions;
- how race results affect the persistent leaderboard and reputation, independently of training
  fidelity.

## Later scope

These are intentionally not prerequisites for the initial headless-to-live sequence:

- race-to-race form variation;
- full pack/group recognition, cooperation and natural passing statistics, once larger fields make
  them visible;
- teams, domestiques, protection and richer classification objectives;
- manual high-level positioning controls;
- reduced-fidelity distant visual populations beyond the benchmarked full-simulation ceiling;
- real-route alignment, wind, crosswind and echelons;
- bit-exact replay of live human telemetry;
- generic unstructured/free rides;
- cloud sync and cross-device leaderboard/history.

## Promotion rule

Before implementing a slice:

1. resolve only its implementation-gating questions;
2. record durable answers in the authoritative architecture or rule document;
3. capture empirical constants as configuration plus tests or baselines;
4. create or refine the slice issue with acceptance criteria from
   [build-order.md](build-order.md);
5. remove resolved items from this file.

This keeps architecture stable, issues executable and the unresolved backlog honest.
