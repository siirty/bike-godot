# Mixed-session experiment

Select **Mixed session** in the workout library. This is a 60-minute software-rider
experiment with mostly endurance work, a three-minute hard contribution, six minutes of
tempo and a one-minute response. See [ADR 0010](../decisions/0010-mixed-session-experiment.md).

## Controls

- **Reference power** supplies the nominal prescribed watts to the software rider.
  Turn it off and enter manual watts to explore missed efforts or extra work. Finite
  physiology limits delivered power; this never writes a target to a trainer.
- Select a lane or named wheel. Only actual eligible geometry supplies shelter.
- Pause with the button or SPACE; restart resets simulation, policies and audit while
  retaining the visible input selections. ×1/×10/×60 execute every fixed kernel step.
- **Export replay trace** writes `user://mixed-race-trace.json` and reports the resolved
  path or an error. **Replay saved trace** validates and replays those controls until
  their recorded end. Restart exits replay mode. Exported data is explicitly simulated.
- The briefing and qualitative roster are always visible. Reveal coaching/private
  diagnostics explicitly to inspect target vs delivered watts, observed race state,
  continuation instruction and NPC requests/reserve.

At 60 minutes the exercise completes irrespective of who leads. The rolling route has
no contested finish. Team affiliations are public metadata here, not a full team/GC model.
A rider who misses a contribution may lose the group; the workout continues without a
free rejoin. Real trainers use the existing separate SIM-only ride screen.

## Headless comparisons

```sh
cargo run -p race_engine --release --example mixed_session -- mixed-race-output
```

Open `mixed-race-output/mixed-session.html`. Compare prescribed input, missed chase,
exposed positioning and a separately authored pre-ride pacer-declines policy. Play/scrub
the hour, inspect actual relative positions and target vs delivered power. `trace.json`
contains the one-second samples and per-block summaries. This is a diagnostic inspector
with private data, not learner assessment or a human activity file.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p race_engine --test mixed_session --release
cargo build -p godot_adapter
godot --headless --import --path apps/rider_godot
godot --headless --path apps/rider_godot res://tests/smoke_test.tscn
# With a display and existing output directory:
godot --path apps/rider_godot res://tests/mixed_screenshots.tscn -- /path/to/screenshots
```

The supporting script is fitted offline against nominal input and frozen before riding.
Matching those prescribed watts is not independent proof that race cues induce the intended
training. The existing all-zone and cue-selection experiments remain separate. Independent
cue execution, general situation compilation, the full state graph, empirical tactical
calibration and physical trainer integration are future work. The [verification record](../verification/2026-10-22-mixed-session.md)
reports measured results and setup limitations.
