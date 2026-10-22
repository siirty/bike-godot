# Mixed-session game prototype

Select **Mixed session** in the workout library. The current world model is provisional.
The director assigns a team objective: keep at least one Ember rider, including you, in
the leading group at the 45-minute competitive deadline. See
[ADR 0011](../decisions/0011-game-session-lifecycle.md).

## Complete session

- **0–10 min:** stationary trainer beside the bus. Read the roster/objective and follow the
  warm-up reference. Opponents do not race during preparation.
- **10–55 min:** the competitive field starts once, carrying your actual preparation reserve.
  Finite capability, frozen terrain and the existing kernel determine movement. Missing an
  opportunity can lose contact without an automatic rejoin.
- **55–60 min:** back to the trainer. The game result and competitive field are frozen while
  player recovery continues. There is a timed team objective, not an invented finish/winner.

**Pause/resume** or SPACE holds the game state and all active/workout clocks. **Skip to cooldown**
advances the workout cursor to minute 55 without adding unperformed work or elapsed active time.
It preserves a paused state and marks the race incomplete/workout skipped. **Finish session** ends
immediately and marks the workout abandoned; a result already resolved at minute 55 is preserved.
Completed means all workout time was executed, not that every power target was hit.

**Reference power** supplies nominal software watts. Turn it off to enter manual power; select a
lane/wheel to explore positioning. Actual eligible geometry supplies shelter. These controls never
write a trainer target. Real trainers retain the separate SIM-only ride screen.

Restart clears dynamic state, audits and outcomes while retaining visible manual/lane/wheel
selections. ×1/×10/×60 execute every 50 ms step. The briefing, phase, objective/result and session
controls remain visible; coaching/private diagnostics require explicit reveal.

**Export replay trace** writes `user://mixed-race-trace.json`. **Replay saved trace** validates
schema 2 / mixed-game-v2, replays controls and lifecycle actions, and stops at recorded active time.
Playback pause excludes wall-time waits; restart exits replay. Old mixed-v1 exports reject. The
file records simulated data only; it is not a human activity file or cross-version replay guarantee.

## Review without riding

```sh
cargo run -p race_engine --release --example game_report -- game-session-output
```

Open `game-session-output/game-report.html`. Compare 12 complete hour-long runs: reference;
missed, delayed and excessive execution of each contribution; 5% easier endurance; ignoring all
contributions. Scrub the timeline or jump to eight checkpoints per variant. Inspect actual gaps,
reserve and power, frozen outcomes, and automatic design findings. Targets/outcomes/private
geometry are visible in this engineering review; it is not a learner assessment. HTML uses rounded,
compact display data; `game-report.json` retains exact numerical samples and `game-report.md` the
summary. The supporting pace is still fitted offline to nominal input, so matching those watts
is not independent evidence of induced demand. Findings remain failures, not silently repaired plans.

The legacy `mixed_session` example and all-zone/cue-selection tools remain available separately.
The [verification record](../verification/2026-10-22-session-lifecycle.md) includes results and limits.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p race_engine --test game_session --release
cargo build -p godot_adapter
godot --headless --import --path apps/rider_godot
godot --headless --path apps/rider_godot res://tests/smoke_test.tscn
# With a display and an existing output directory:
godot --path apps/rider_godot res://tests/mixed_screenshots.tscn -- /path/to/screenshots
```
