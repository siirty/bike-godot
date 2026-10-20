---
description: Numerics and testing conventions for the pure physics crate
paths:
  - "crates/race_physics/**"
---

# race_physics conventions

- **Pure Rust only.** No Godot, no BLE, no I/O, no wall clock. If a change needs any of them, it
  belongs in another crate.
- **The power→speed solver is a safeguarded Newton–Raphson with bisection fallback**, bracketed to
  `v ∈ [0, ~30 m/s]` (`MAX_SOLVABLE_SPEED_MS`). It is not an ad hoc iteration. Keep it that way:
  this is what guarantees convergence for physically sane inputs.
- **Forces are individually testable public functions**: `gravity_force_n`,
  `rolling_resistance_force_n`, `aero_drag_force_n`. Do not fold them into one opaque
  calculation. Separate functions are what make independent reference checks possible
  (`tests/independent_reference.rs`).
- **`RiderPhysicalParams` owns its JSON** through its serde derive. `race_engine`'s snapshot codec
  and determinism comparison both go through that mapping rather than naming the fields again, so
  *adding a parameter here reaches them*. This is the same ownership rule `workout_import` follows
  for segment JSON.
- **The production dynamics step is one fixed semi-implicit Euler step.** No adaptive stepping, no
  substeps inside `step_longitudinal_dynamics`, and no "just refine it when the acceleration is
  large". A race's trajectory must depend on its inputs, not on how hard the arithmetic was. A
  finer step is a *measuring stick* (the `convergence` module, behind the `testing` feature),
  never a refinement of the production one.
- **The timestep is an argument.** `SimulationTimestep::RECOMMENDED` (50 ms) is the value that was
  actually measured. Changing the step, the integrator or the convergence corpus means:
  1. re-running `cargo run -p race_physics --features testing --example convergence_report
     --release`;
  2. updating the budget constants on `SimulationTimestep`.

  `tests/convergence.rs` will insist on it. Drafting thresholds are designed against those numbers.
- **Numerical, non-physical inputs travel in `LongitudinalDynamicsConfig`, explicitly.** Today that
  is the low-speed tractive-force cap. Do not demote one to a private constant: an input that does
  not appear in the simulation's configuration does not appear in its snapshot either. A patch
  release could then silently change how every saved race resumes.
- **Drafting enters the model as `aerodynamic_drag_multiplier`, and it scales the aerodynamic term
  only.** Never let shelter reach power, mass, rolling resistance, grade or speed directly. That is
  the difference between a drafting model and a rubber-banding lever.
- **Public entry points validate their whole input before computing anything.** They return
  `PhysicsError::InvalidArgument`, naming the caller-facing field. `race_engine` owns typed
  simulation validation, and duplicating it here would make two crates disagree about which values
  are legal.
- **Behaviour is pinned by `tests/golden.rs`** against `fixtures/golden/race_physics.json`,
  recorded from the Dart prototype, bit for bit. A deliberate change of behaviour is listed in
  `docs/port/divergences.md` and named in the test. Otherwise the test is right, not the change.
- **Never re-derive the human rider's speed independently once real trainer telemetry is
  available.** The solver and the dynamics step exist for three things:
  - AI riders, who have no physical trainer;
  - smoothing the human's position between the trainer's ~1–4 Hz reports;
  - pre-ride terrain planning.

  They are not for double-simulating a human whose trainer already computes this. See
  `docs/architecture/physics-design.md`.
