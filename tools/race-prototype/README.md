# All-zone race transition laboratory

A runnable experiment in deterministic pre-ride race-action scheduling, using the **existing
production Rust race kernel at 50 ms**. It does not change Godot, trainer control or production
orchestration. The fixture covers Z1–Z7 over **17 minutes**, with 13 phases, repeated recovery,
a finishing sprint and cooldown. Zones and synthetic capabilities are explicit engineering
fixtures, not a training recommendation or a validated physiological interpretation.

## Run

From the repository root, with the normal Rust toolchain and access to the pinned trainer-core
dependency:

```sh
cargo run -p race_prototype --release -- --out race-prototype-output
# Optional: different nominal rider and deterministic profile seed
cargo run -p race_prototype --release -- --ftp 320 --mass 92 --seed 7 --out another-run
```

Open `race-prototype-output/race-prototype.html` in a browser. It is self-contained and needs no
server or network. Select a test policy, play/scrub, jump to phase boundaries, and inspect the
observed scene, positions, power, reserve, terrain and transition audit. Hide authored cues to
inspect the constellation without being told which planned scene should occur. Per-rider rows
are a longitudinal diagram, not physical road lanes.

Exports:

- `report.json`: settings, workout, terrain, capabilities, per-second traces, debounced observed
  scene transitions and per-phase diagnostics for all seven replay modes.
- `frozen-opponent-script.json`: exact power/braking/slot inputs for every opponent at 20 Hz.
- `samples.csv`: effort, demand, gaps and reserve for external analysis.
- `race-prototype.html`: embedded-data playback viewer.

## What is frozen

An offline calibration pass adjusts ordinary opponent intentions to phase-specific geometry
around one prescribed-power reference rider. Only the kernel advances physical state. The compiled
opponent inputs, field, initial state, finish distance and distance-based terrain are then frozen.
Replays never change the opponent script in response to the athlete, reset between phases,
teleport riders, change capability or invoke a runtime Scene Planner.

The reference fixture uses CP=FTP, W′=20 kJ and peak power=4.5 FTP, with explicitly weaker/stronger
opponents. There is no claim that this describes a particular athlete. The road is generated in
distance coordinates; slower/faster riders may reach its changes at different workout times.

## Read the results correctly

The scheduled label is an intention. **Observed scene** is a separate geometry-only classification
with two-second persistence: in-group, sheltered, leading, chasing, detached behind, or solo ahead.
The diagnostic thresholds are defined in `classify`, not taken from the workout phase.

The per-phase gates are provisional: ≥80% of time within ±10% FTP of the prescription, mechanical
pace demand within ±20% FTP, intended formation available ≥50% of the phase, and physiology
shortfall ≤1% of requested work. Formation is a named usable wheel in Z1/Z2, a pacer behind in
Z3, or a strong move ahead within 25 m during higher zones. These thresholds do not prove natural
race behavior, necessity of the effort, or tactical significance. Both false passes and false
failures should be reviewed in playback.

Mechanical demand estimates the crank watts required for the athlete to match the named target's
actual speed/acceleration under the athlete's current shelter and terrain. It is separate from
prescribed input; it is not a complete strategic feasibility solver. The simple named-wheel
controller also receives no prescribed watts. Ignore/cover-decoy outcomes help inspect the cost
and consequences of responding to an authored low-stakes move; the label alone proves nothing.

## First measured result (250 W, 85 kg, seed 42)

| Test rider | Phases passing provisional gates |
|---|---:|
| Prescribed / ignore decoy | 13/13 |
| 5% under | 1/13 |
| 5% over | 0/13 |
| Three-second late changes | 1/13 |
| Simple named-wheel follower | 1/13 |
| Cover recovery decoys | 4/13 |
| Deliberately miss Z6 bridge | 7/13 |

Nominal success is expected after fitting the nominal trajectory. **It does not establish
robustness.** Small power differences lose shelter or change gaps, which then propagate through
later phases. The current template/controller/geometry gates are brittle. This is evidence about
this concrete prototype, not a proof that every fixed-schedule design needs live replanning.
Inspect whether a failed gate means a genuinely wrong training opportunity or merely a different
but acceptable constellation before changing the design.

The report does not record real activity. There is no BLE integration, physical-trainer test,
validated TrainingContract evaluator, general generator, tactical intelligence or collision
avoidance in this tool.

## Verification

```sh
cargo test -p race_prototype --release
cargo clippy -p race_prototype --all-targets -- -D warnings
cargo fmt --all --check
```

The experiment tests cover all-zone continuity, deterministic scripts and replay, unchanged
opponent inputs, non-resetting consequences, independent action-following behavior, extra decoy
work, invalid settings and a small profile/seed matrix. They do not turn failed behavioral gates
into software-test failures: exposing those failures is the purpose of the experiment.

Validated with seven experiment tests and 116 existing domain tests passing, Clippy without
warnings, formatting checks, and the four unchanged fixture checks. Browser checks exercised
mode selection, phase navigation, scrubbing, cue hiding and playback; the mobile layout had no
page overflow. Godot rendering and physical trainer hardware were not exercised.
