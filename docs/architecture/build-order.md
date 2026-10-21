# Build order (vertical slices)

Each slice produces an increment that can be executed and tested on its own. For the headless
slices, "end-to-end" means through the simulation domain, not through the trainer or the UI. Read
[physics-design.md](physics-design.md), [product-alignment.md](product-alignment.md) and
[open-race-design-questions.md](open-race-design-questions.md) before implementing Slices 5–9.

Risk drove the order. The risky layer (trainer I/O, physics, the deterministic foundation) is
built. The differentiating layer is partly built: spotlight tactics now run, while training contracts,
situation synthesis, player context and independent learning/result validation remain.
[ADR 0007](../decisions/0007-tactical-training-core.md) replaces the mandatory Scene Planner slice
with director-first situation planning. [TT-01–TT-10](../requirements/tactical-training.md) are the
acceptance requirements; Slices 5–9 are the remaining core loop, not a backlog of polish.

0. **Repo scaffolding** *(done)*: Cargo workspace, Godot project, quality CI on the self-hosted
   runner.
1. **Import → static display** *(done)*: `.zwo`/`.erg`/`.mrc` → `WorkoutPlan`, saved by
   `local_storage`, listed and shown in the app.
2. **Ride against a simulated trainer** *(done)*:
   - `TrainerControl` with the simulated trainer;
   - `RideSession` over the synthesized route;
   - the Godot ride screen, with HUD, route strip and road view.

   This proves the full data pipeline with no BLE risk.
3. **Real FTMS hardware via `trainer_core`** *(built, not validated on hardware)*: `ble_trainer`
   implements `TrainerControl` over `trainer_core` (`btleplug`). It covers scan, connect,
   telemetry and SIM parameter writes, and has no ERG path.

   The JetBlack Victory SIM-mode spike still has to run on the Linux ride computer. It records
   gradient caps, parameter units, resistance smoothing and latency into
   [`docs/trainers/`](../trainers/). Claiming multi-brand support requires ≥2 real trainer models.
   The non-Linux builds are listed in [platform-rollout.md](platform-rollout.md).
4. **Deterministic simulation foundation** *(done)*: the pure `RaceSimulation::step(state, inputs,
   dt)` kernel. It has:
   - explicit simulation time, `pcg32-v1`, typed validation and resumable snapshots;
   - semi-implicit longitudinal dynamics, 1.5-D slots and basic drafting;
   - typed race events.

   The headless harness (`race_engine::harness`, behind the `testing` feature) generates seeded
   races and runs them to the finish, checking invariants every step. To print the report, run:

   `cargo run -p race_engine --features testing --example headless_corpus --release -- 0 32`
5. **Headless autonomous race** *(physiology, pack pacing and initial spotlight tactics built; calibration pending)*: finite
   CP/W′/Pmax, Skiba differential recovery, reserve-limited sprints and seeded pack plans run in
   the headless corpus and live demo. Snapshot v3 carries all capability and reserve state.
   `TacticalController` supplies reactive recovery/follow/chase/attack/sprint intentions to the live
   demo and the mixed-field headless corpus. Run:

   `cargo run -p race_engine --features testing --example tactical_corpus --release -- 16`

   The report records action occupancy, not empirical naturalness. Remaining work:
   - calibrated rider profiles and tactical personality distributions;
   - empirical tuning of the implemented spotlight utilities and reaction behavior;
   - expand and calibrate the existing capability-limited pack pace templates;
   - connect the bounded tactical-role seam to future scenario/scene goals when Slice 6 provides them.

   Full pack/group recognition, cooperation and natural passing statistics wait until larger fields
   make them visible. Validate believable spotlight behavior, determinism, invariants and long-run
   numerical stability across seeds. Use the existing baseline for focused situation validation;
   do not make comprehensive autonomous whole-race realism a prerequisite for the learning vertical.
6. **Threshold vertical: contract → tactical situation → evaluator.** Authoring/public-knowledge
   contracts, a cue reader and separate diagnostic exports are built in the all-zone experiment
   ([ADR 0008](../decisions/0008-learning-contracts-and-public-reader.md)); general compilation,
   calibrated evaluation and human acceptance evidence remain. One thin vertical for
   a single workout type, including the first cut of Slices 7 and 8:
   - derive a versioned contract from immutable workout data;
   - preplan terrain, relevant riders/affiliations, player objective/knowledge and bounded actions;
   - specify readable cues, response window, acceptable execution, demand and exit conditions;
   - run continuously on the same kernel, without ordinary-error rescue;
   - compare prescribed-power control with an independent cue/action-following policy;
   - inspect workout fidelity, recognition/timing, continuity and path-to-result plausibility.

   A dogfooder ride with essential context available and coaching optionally hidden supplies early
   evidence. Completion requires the focused TT acceptance evidence, not merely a nominal watt fit.
   The existing all-zone prototype is an experimental diagnostic, not completion of this vertical.
7. **Generalize contracts and tactical situation validation**:
   - extend mappings and interval/block explanations, with FTP tests as time trials;
   - add state projections, accepted response/formation alternatives and context-transfer tests;
   - validate low-threat recovery movement and connected execution through reasonable variations;
   - keep local tactical decisions at a slower explicit cadence than physics;
   - calibrate proficiency assumptions and result plausibility against appropriate evidence.

   A general runtime Scene Planner and automatic rider-error repair are not prerequisites.
8. **Full Workout Director**:
   - compose the whole workout into connected tactical learning situations;
   - freeze generated terrain and field capabilities;
   - compile bounded opponent action/role plans and entry/exit/opportunity windows;
   - provide relevant roster, affiliations, known strengths and player objectives;
   - check future opportunities within explicit reference bounds and credible path to contention;
   - reject/regenerate infeasible candidates before riding, or offer an explicit non-race alternative.

   Expand the reference-policy and seed corpus as focused cases become credible. Exact scheduling
   anchors, compute budget and production schemas remain open.

9. **Human, trainer and race UI integration**:
   - turn real trainer telemetry into rider inputs, and drive the same kernel from the Godot app;
   - add technical-interruption pause semantics, SIM parameter output, manager radio and event
     presentation, roster/briefing context and configurable coaching/HUD guidance;
   - keep measured and simulated data separate;
   - offer the athlete the ridden-versus-simulated finish choice.

   The Slice 2 `RideSession` is reused behind these adapters rather than becoming the simulation
   core.
10. **Real-route alignment and richer race mechanics**:
    - `.gpx`/`.tcx`/`.fit` course parsing and `TerrainAlignmentStrategy`;
    - wind, crosswind and echelons;
    - richer drafting and race objectives, when the validated core justifies them.
11. **Future**: cloud sync, a persistent cross-device leaderboard and history, and calendar or
    platform integrations, unless product validation pulls them forward.

## Verification per slice

- **Slices 0–3**:
  - unit and golden-vector tests stay green;
  - the Godot smoke test stays green;
  - real FTMS hardware remains a manual QA gate.
- **Slice 4**:
  - a fixed seed plus fixed inputs produce the same state and event sequence;
  - dynamic-physics tests cover acceleration, coasting, braking and drafting;
  - the corpus runs every seed to the finish without breaking an invariant;
  - its events agree with reconstructions from the states: net overtakes equal the change in
    running order, and replayed wheel and slot events reproduce the final state;
  - a race resumed from a snapshot reports exactly the remaining events.
- **Slice 5**:
  - headless races across a seed corpus report pack stability, attacks, splits, catches and finish
    distributions;
  - no Godot, wall-clock timer or trainer dependency is reachable from the kernel.
- **Slice 6**:
  - deterministic threshold contract derivation, situation generation and effort evaluation;
  - a cue/action-following policy independent of target watts and a prescribed-power control;
  - rider review of recognition/timing with essential context available and optional hints hidden;
  - observed local consequences and plausible course/field progression.
- **Slice 7**:
  - contract derivation for every mapping, including interval/block explanations and FTP test → TT;
  - context-sensitive and identity-transfer cases, ignore/cover comparisons and plausible selection/
    finishing cases; reported uncertainty and calibration limits;
  - continuous reference variations distinguish valid alternatives from lost opportunities.
- **Slice 8**:
  - generated plans pass separate workout, learning, continuity and race-plausibility checks;
  - failures remain recorded; no hidden runtime repair or predetermined winner;
  - larger corpora report validity and generation rejection/regeneration rates.
- **Slice 9**:
  - integration tests prove the live adapter drives the same kernel;
  - technical pauses do not create race consequences;
  - simulated outcomes never enter measured telemetry or history.
- **Slice 10+**: route alignment and richer mechanics get focused golden/statistical tests, without
  weakening the earlier simulation invariants.
