# Product alignment

Product direction was revised on 21 October 2026 by
[ADR 0007](../decisions/0007-tactical-training-core.md). It supersedes the earlier equal-priority
full-race framing and mandatory live scene-repair plan from 21 September. The authoritative
behavior requirements are [TT-01–TT-10](../requirements/tactical-training.md). Update this document
and its dependents together when these decisions change.

## Product identity

A **structured workout trainer that teaches the tactical eye through credible race situations**.
The athlete learns when to commit power, conserve, follow, or let others work. The workout defines
the stimulus; the situation supplies a recognizable reason to execute it. The app does not aim to
replicate full races, and overall placing is not the primary training objective.

Believability remains a requirement at both scales: local actions must be readable and credible,
and the connected selections, fatigue and any finishing result must make sense for the course and
field. Race excitement supports learning and adherence; it does not justify an invalid workout,
unreadable lesson or predetermined winner.

For the current designed-race prototype, [ADR 0011](../decisions/0011-game-session-lifecycle.md)
adopts explicit game rules while the world model remains provisional. Prioritise a complete
warm-up/race/cooldown loop, meaningful choices and workout alignment. Empirical race prediction
and tactical-transfer claims wait for world-model and human validation. Game success alone does
not establish that the intended racing action induces the intended workout.

Priority is given to:

1. **Tactical readability and fair information**: the player can judge a move from known context
   and observable behavior, with optional coaching hidden.
2. **Workout-correct action opportunities**: appropriate racing actions produce the planned effort
   and recovery within declared bounds.
3. **Credible continuity and consequences**: reasonable execution enables later situations;
   mistakes have physical consequences; the path to a finishing result remains plausible.

A complete peloton, career system and broad autonomous-race realism are not prerequisites. Build
only the roster, affiliations and behavior needed to convey and validate the lesson.

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

## Situation planning and opponent intelligence

The initial approach is **director-first deterministic planning**, together with frozen terrain
and fixed field capability. It defines a connected sequence of tactical opportunities, bounded
opponent intentions/roles and player context before the ride. The exact timeline/distance/event
anchors and scenario schema remain open.

The implemented two-tier AI remains useful: spotlight riders have local reactive tactics; pack
riders can use capability-honest pace templates. All share the same kernel and finite physiology.
There is no requirement to grow the whole field into utility AI. Escalate complexity only when a
lesson's observable behavior or result plausibility needs it.

Pre-ride plans may constrain actions to create workout-compatible opportunities. Local AI receives
observable race state and bounded intentions, not target error, fidelity or a lesson's hidden
answer. The current prototype additionally freezes exact commands after offline fitting; that is
an experimental policy, not the mandatory production implementation.

A general runtime Scene Planner and automatic recovery from athlete mistakes are not initial
requirements. Validate the sequence for bounded reference execution; retain actual consequences
outside it. Changes to this policy need explicit failure evidence and a decision.

Basic recognizable rider identities, relevant strengths, affiliations and the player's objective
are initial scope wherever a lesson depends on them. A large roster, exact capability readouts,
database persistence and sophisticated team coordination are not prerequisites.

## One thin vertical, then generalize

Contracts, scenarios and the evaluator are built as **one thin vertical for one workout type**,
end to end. Generalizing to every type comes after. The mapping table in `physics-design.md`
eventually covers every workout type. The cheapest way to learn what a contract must contain is to
run one type through the whole loop and ride it. The vertical includes tactical recognition,
player context, continuity and result plausibility, not only target-power delivery.

- **The anchor type is threshold.** It maps to chase, bridge, paceline and long-climb situations.
  It is the bread-and-butter winter interval, and its power envelope is well defined.
- **Generalizing** to the remaining mappings follows once the first vertical survives a real ride.
- **Validation starts focused**: deterministic generation and physical feasibility, an independent
  cue/action-following policy, bounded execution variations and human review of the intended move.
  Keep workout, recognition, continuity and result checks separate. A dogfooder ride supplies early
  evidence, not proof of general learning transfer. Broader proficiency/seed corpora follow when
  the focused cases are credible. The workout always anchors generation.

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
