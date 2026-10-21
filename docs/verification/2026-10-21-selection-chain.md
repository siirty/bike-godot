# Cue execution and connected selection verification — 21 October 2026

## Scope and decisions

[ADR 0009](../decisions/0009-cue-calibrated-selection-chain.md) records the implementation choices.
The new focused exercise uses the production 50 ms kernel and a frozen 1.5% climb:

| Block | Start | Duration | Reference effort |
|---|---:|---:|---:|
| Hold the initial group | 0 s | 30 s | 65% FTP |
| Follow the sustained selection | 30 s | 90 s | 100% FTP |
| Shelter while Jo takes over | 120 s | 120 s | 45% FTP |
| Follow the next selection | 240 s | 90 s | 100% FTP |

The exercise starts in one common, already moving group. This initial state is fitted offline;
there are no subsequent resets. The first block is evaluated, not exempted. Opponents receive
frozen pace/leader/offset roles and actual motion, without workout targets or athlete error. The
athlete uses public motion, roster/objective and its own fixed capability. Target/answer mutation
tests verify those labels cannot alter the replay. Terrain, capability and pace intentions stay
unchanged across all variants. No new dependencies, kernel formulas, snapshots or fixtures changed.

The motor executor now distinguishes lateral files, calculates actual drafting and issues ordinary
power/brake requests. Controlled tests cover equilibrium, closing a gap, downhill slowing, invalid
inputs and launch from rest. Reader tests include an established sprinter gap and a teammate losing
the wheel: reputation and approximate proximity alone cannot establish harmlessness or coverage.
The all-zone physical response proxy now requires one second closing/contact, rather than a power
rise. Neither proxy measures human recognition.

## Default results, 250 W / 85 kg / seed 42

Percentages are actual 20 Hz delivered samples within ±10% FTP. Each of the first three execution
variants must achieve at least 80% in **every** block, have an available same-file wheel within
8 m at the second selection, finish at 323–335 s and keep either pace leader's clipping ≤100 J.
The line is fixed from the offline reference before any bounded/error replay.

| Execution | Initial group | First selection | Recovery | Second selection | Actual finish |
|---|---:|---:|---:|---:|---:|
| Public cues | 100% | 84.89% | 87.54% | 80.17% | 330.00 s |
| One-second held commands | 100% | 84.44% | 85.00% | 80.00% | 330.05 s |
| Alternating ±3% output | 100% | 84.72% | 87.50% | 80.44% | 330.00 s |
| 5% less motor output | 100% | 84.28% | 87.83% | 78.94% | 330.05 s |
| 5% more motor output | 100% | 85.33% | 85.00% | 81.22% | 330.00 s |
| Skip second selection | 100% | 84.89% | 87.54% | 0% | Not reached by 335 s |
| Cover other-file movement | 100% | 84.89% | 81.42% | 80.44% | 330.50 s |

The default plan passes its **declared** bounds. The second-selection margins are narrow; this is
not robust outside those bounds. A 5% reduction of feedback-controller output is not a sustained
5% reduction of delivered effort: feedback partly compensates by allowing a slightly larger gap.
Skipping the second selection leaves a 332.4 m deficit versus 12.0 m for reference execution,
with no rescue. Covering costs approximately 0.19 kJ more overall and finishes 0.5 s later here;
that is a fixture observation, not a universal penalty for covering safe movement.

## Profile matrix

27 plans: FTP 180/250/330 W × total mass 65/85/110 kg × seed 1/42/99. Seven comparison replays per
plan, **189 evaluated replays**, in addition to bounded offline fitting passes. No execution errors.

| FTP | 65 kg: accepted / 3 seeds | 85 kg | 110 kg |
|---|---:|---:|---:|
| 180 W | 3/3 | 3/3 | 3/3 |
| 250 W | 0/3 | 3/3 | 3/3 |
| 330 W | 0/3 | 0/3 | 3/3 |

**18 plans accepted, 9 rejected.** All rejected plans fail at least one actual effort envelope;
next-entry contact and finish timing still pass. Scheduled pace leaders remain within the capacity
gate. The fixed transition/role template is insufficient at some higher power-to-mass ratios. Do
not publish those plans as suitable workouts or infer that every director-first approach fails.
This experiment deliberately records rejection rather than widening tolerances or repairing a ride.

The weak sustained-capacity adversarial test keeps identity and role unchanged, lowers only its
explicit pre-ride CP/W′ fixture and verifies actual climbing clipping and failure to reach the
line. Climbing work, clipping and actual events are course-conditioned mechanical evidence, not an
empirical race-result score. No rider archetype is banned from winning.

## Original all-zone comparison

The seven phase-informed controls preserve their physical trajectories. The changed cue-driven
motor meets 5/13 workload phases (4/12 non-cooldown), versus 3/13 previously, and finishes at
970.8 s, after the authored 945–960 s sprint window. Report version 3 explicitly rejects that
cue/workout fit. Improvement in the motor does not make the all-zone template acceptable.

## Checks and reproduction

- 151 Rust tests across the six domain/prototype packages, including focused controller, perception,
  physical-response timing, acceptance rejection, label independence and course-capacity cases.
- Clippy over those packages/all targets with warnings denied; root formatting and diff checks.
- Four unchanged frozen fixture files checked by `scripts/check-fixtures.py`.
- Both viewers: policy selection, context, hidden diagnostics, navigation/scrubbing/playback,
  actual-event timing and 390 px mobile width. No JavaScript errors. Screenshots inspected.
- New focused viewer renders actual lateral files and retains visible names/team context with
  diagnostics hidden. Original viewer retains its explicitly longitudinal, per-rider rows.

The private trainer dependency remains unavailable here. Rust checks use exact copies of the six
packages and the repository's pinned lockfile with unused workspace members excluded. Godot, BLE,
storage and physical trainer hardware are untested. Normal repository commands:

```sh
cargo test -p race_engine -p race_physics -p session_director -p workout_import -p trainer_control -p race_prototype --release
cargo clippy -p race_engine -p race_physics -p session_director -p workout_import -p trainer_control -p race_prototype --all-targets -- -D warnings
cargo run -p race_prototype --release --example chain -- connected-selection.json
cargo run -p race_prototype --release --example chain -- chain-matrix.json matrix
```

## Remaining work

This supplies a limited connected **group-selection** case. It does not implement an intuitive
public tempo pull-duty cue, player-led rotation, a calibrated finishing sprint, varied-course
synthesis, proficiency models, exhaustive result plausibility or human recognition/transfer.
Human practice observations remain unscored. The next generation work should address rejected
transition templates and add terrain/context transfer cases before expanding the all-zone plan.
