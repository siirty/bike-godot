# Build order (vertical slices)

Each slice produces an increment that can be executed and tested on its own. For the headless
slices, "end-to-end" means through the simulation domain, not through the trainer or the UI. Read
[physics-design.md](physics-design.md), [product-alignment.md](product-alignment.md) and
[open-race-design-questions.md](open-race-design-questions.md) before implementing Slices 5–9.

Risk drove the order. The risky layer (trainer I/O, physics, the deterministic foundation) is
built. The differentiating layer is not built yet: training contracts, scenario synthesis, the
scene planner and tactical AI. Slices 5–9 are the remaining core loop, not a backlog of polish.

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
4. **Deterministic simulation foundation** *(mostly done)*: the pure `RaceSimulation::step(state, inputs,
   dt)` kernel. It has explicit simulation time, `pcg32-v1`, typed validation and resumable
   snapshots, plus semi-implicit longitudinal dynamics, 1.5-D slots and basic drafting.

   **Remaining:**
   - the real `RaceEvent` vocabulary, replacing the `Reference` placeholder;
   - a headless harness that runs a fixed seed corpus and reports stability.
5. **Headless autonomous race**: run fully simulated fields on hand-authored terrain and initial
   conditions. This needs:
   - rider fixtures with CP, W′, Pmax/decay, physical parameters and tactical personality;
   - utility-based tactics for the 2–3 drama-spotlight riders;
   - capability-honest scripted pace plans with seeded variance for the pack;
   - a minimal scene-goal seam, so the spotlight AI has something to have intent about.

   Full pack/group recognition, cooperation and natural passing statistics wait until larger fields
   make them visible. Validate believable spotlight behavior, determinism, invariants and long-run
   numerical stability across seeds. Do all of this before adding workout orchestration or a human
   rider.
6. **Threshold vertical: contract → scenario → evaluator.** This is one thin vertical for a single
   workout type, end to end; it absorbs the first cut of Slices 7 and 8. Threshold is the anchor
   because it maps to chase, bridge, paceline and long-climb situations, and its power envelope is
   well defined. The vertical:
   - derives a versioned `TrainingContract` from the immutable `WorkoutTimeline`;
   - has `session_director` generate a scenario anchored to that contract;
   - runs it on the same headless kernel;
   - evaluates it with the evaluator that later reports post-ride fidelity.

   Validation is light: deterministic generation plus a feasibility smoke-check, meaning the
   scenario is completable and the field coherent. The vertical is done when a dogfooder has ridden
   it.
7. **Generalize contracts and the Scene Planner**:
   - extend contracts to all mappings in physics-design.md's table, keeping interval/block
     explanations, with FTP tests as time trials;
   - grow the planner to semantic race-state projections, bounded tactical roles/influence, and
     live fallback selection;
   - run tactical decisions at a slower explicit cadence than the physics step.
8. **Full Workout Director**:
   - whole-session outline;
   - frozen generated terrain;
   - field composition and persistent rider selection;
   - objectives and acceptable scene regions;
   - structured non-race fallbacks when no feasible pack race exists.

   Forward-running validation across bounded under/at/over-target controllers and seed corpora is
   added when it is shown to catch real failures.
9. **Human, trainer and race UI integration**:
   - turn real trainer telemetry into rider inputs, and drive the same kernel from the Godot app;
   - add technical-interruption pause semantics, SIM parameter output, manager radio and event
     presentation, and configurable HUD guidance;
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
  - long accelerated runs remain finite and keep the stated invariants.
- **Slice 5**:
  - headless races across a seed corpus report pack stability, attacks, splits, catches and finish
    distributions;
  - no Godot, wall-clock timer or trainer dependency is reachable from the kernel.
- **Slice 6**:
  - golden tests assert deterministic contract derivation, scenario generation and evaluator
    results for canonical threshold workouts;
  - a dogfooder ride confirms the vertical.
- **Slice 7**:
  - golden tests cover contract derivation for every mapping, including interval-level explanations
    and FTP test → TT;
  - seeded tests cover tactical choices, coordinated roles, fallback selection, and the rule that
    AI-driven transitions keep viable continuations without mutating capability.
- **Slice 8**:
  - generated scenarios pass the feasibility smoke-check;
  - once the heavy battery exists, it reports reachability, fidelity and regeneration/fallback
    rates.
- **Slice 9**:
  - integration tests prove the live adapter drives the same kernel;
  - technical pauses do not create race consequences;
  - simulated outcomes never enter measured telemetry or history.
- **Slice 10+**: route alignment and richer mechanics get focused golden/statistical tests, without
  weakening the earlier simulation invariants.
