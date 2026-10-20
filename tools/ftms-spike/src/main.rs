//! FTMS SIM-mode write-path spike — JetBlack Victory on Linux desktop.
//!
//! A throwaway experiment runner, NOT adapter code (see
//! `docs/trainers/jetblack-victory.md`, whose verification log this feeds). It drives the
//! same `trainer_core` seams the app uses — `TrainerConnection::connect` /
//! `set_sim_params` / `next_telemetry` — so what it observes is what the ride loop will
//! see, and it prints markdown-ready rows for the datasheet's verification log.
//!
//! Two backends:
//! - default: real BLE via BlueZ (`--name` filters the advertised name, default VICTORY)
//! - `--sim`: the core's `SimulatedTrainerConnection`, for CI/rehearsal without hardware.
//!
//! Experiments (`--exp NAME`):
//! 1. `ramp` — grade accepted at 0%? ramp 0→…→20→-5%; find the cap and above-cap behavior.
//! 2. `units` — wind speed sign check and Crr/Cw step check.
//! 3. `latency` — grade steps; write vs report timestamps for latency/smoothing analysis.
//! 4. `sustain` — scripted terrain profile with coast-down probes for the known
//!    power-floor flaw (GPLama 2024-10).
//! 5. `telemetry` — trainer-reported power vs the app's power model per grade; feeds
//!    Slice 9's measured-vs-simulated separation.
//!
//! The power model in experiment 5 is deliberately duplicated here (documented, small,
//! spike-scoped): the riding model P = F_total·v with slope/rolling/aero terms, solved
//! for the speed that balances the commanded grade at a reference power. It stays local so the
//! spike depends on nothing but `trainer_core`.

use std::time::{Duration, Instant};

use trainer_core::ble::{ConnectOptions, TrainerConnection as BleConnection};
use trainer_core::simulation::{RiderParams, SimulatedTrainerConnection};
use trainer_core::transport::TrainerConnection as TrainerConnectionTrait;
use trainer_core::units::{DragArea, GradeFraction};
use trainer_core::SimParams;

/// The Victory's advertised name starts with this; overridable for other trainers.
const DEFAULT_NAME_FILTER: &str = "VICTORY";

/// Default device cap recorded in `trainer_core::ftms::device_quirks` — experiment 1
/// exists to verify (or correct) this exact constant on real firmware.
const EXPECTED_MAX_GRADE_PERCENT: f64 = 16.0;

/// App policy (`.claude/rules/trainer-control-ble.md`): SIM writes at ~1 Hz max.
const WRITE_PACING: Duration = Duration::from_millis(1100);

/// Rider used for the physics cross-check (experiment 5) and the simulator backend.
const RIDER: RiderParams = RiderParams {
    total_mass_kg: 83.0,
    crr: 0.005,
    cda_m2: 0.32,
};

const AIR_DENSITY_KG_M3: f64 = 1.225;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse(std::env::args().skip(1).collect());
    if let Err(err) = args.run().await {
        eprintln!("\nSPIKE ERROR: {err}");
        std::process::exit(1);
    }
}

/// Parsed CLI. Deliberately hand-rolled — a spike does not need clap.
struct Args {
    experiment: Experiment,
    backend: Backend,
    name: String,
}

enum Backend {
    Ble,
    Sim,
}

enum Experiment {
    Ramp,
    Units,
    Latency,
    Sustain,
    Telemetry,
}

impl Args {
    fn parse(argv: Vec<String>) -> Self {
        let mut experiment = Experiment::Ramp;
        let mut backend = Backend::Ble;
        let mut name = DEFAULT_NAME_FILTER.to_string();
        let mut iter = argv.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--exp" => match iter.next().unwrap_or_default().as_str() {
                    "ramp" => experiment = Experiment::Ramp,
                    "units" => experiment = Experiment::Units,
                    "latency" => experiment = Experiment::Latency,
                    "sustain" => experiment = Experiment::Sustain,
                    "telemetry" => experiment = Experiment::Telemetry,
                    other => {
                        eprintln!("unknown --exp '{other}' (ramp|units|latency|sustain|telemetry)");
                        std::process::exit(2);
                    }
                },
                "--sim" => backend = Backend::Sim,
                "--name" => name = iter.next().unwrap_or_else(|| DEFAULT_NAME_FILTER.into()),
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => {
                    eprintln!("unknown argument '{other}' (see --help)");
                    std::process::exit(2);
                }
            }
        }
        Self {
            experiment,
            backend,
            name,
        }
    }

    async fn run(&self) -> Result<(), String> {
        match &self.backend {
            Backend::Sim => {
                // The simulator reports one row per 500 ms tick once started; same trait
                // surface as BLE, so the experiment code below is identical for both.
                // The `as_any` seam reaches the simulator's rider-power input — on a real
                // trainer this is the rider's legs, which the spike asks the human for.
                let sim = SimulatedTrainerConnection::new(RIDER, Duration::from_millis(500), 1);
                sim.set_rider_power_w(150.0);
                let mut conn: Box<dyn TrainerConnectionTrait> = Box::new(sim);
                conn.start_or_resume()
                    .await
                    .map_err(|err| format!("sim start failed: {err}"))?;
                println!("backend: simulated trainer (no hardware)");
                run_experiment(&self.experiment, &mut conn, true).await
            }
            Backend::Ble => {
                println!(
                    "scanning for an FTMS trainer whose name starts with '{}'",
                    self.name
                );
                let options = ConnectOptions {
                    scan_timeout: Duration::from_secs(20),
                    ..Default::default()
                };
                let ble = BleConnection::connect(
                    |name| name.to_uppercase().starts_with(&self.name.to_uppercase()),
                    options,
                )
                .await
                .map_err(|err| format!("connect failed: {err}"))?;
                println!("connected; FTMS handshake complete (Request Control + Start/Resume)");
                let mut conn: Box<dyn TrainerConnectionTrait> = Box::new(ble);
                run_experiment(&self.experiment, &mut conn, false).await
            }
        }
    }
}

/// Shared dispatch so `--sim` rehearses exactly what BLE will do; only the capability
/// gate is relaxed for the simulator (which does not implement the FTMS feature read).
async fn run_experiment(
    experiment: &Experiment,
    conn: &mut Box<dyn TrainerConnectionTrait>,
    sim: bool,
) -> Result<(), String> {
    let caps = conn.capabilities();
    println!("## Capabilities (0x2ACC)");
    println!(
        "- sim_mode={} power_target={} resistance_target={} power_meas={} cadence={}",
        caps.supports_sim_mode,
        caps.supports_power_target,
        caps.supports_resistance_target,
        caps.supports_power_measurement,
        caps.supports_cadence_measurement,
    );
    if !caps.supports_sim_mode && !sim {
        return Err(
            "trainer does not advertise SIM-mode support (0x2ACC bit 13) — aborting".into(),
        );
    }

    match experiment {
        Experiment::Ramp => experiment_ramp(conn).await,
        Experiment::Units => experiment_units(conn).await,
        Experiment::Latency => experiment_latency(conn).await,
        Experiment::Sustain => experiment_sustain(conn).await,
        Experiment::Telemetry => experiment_telemetry(conn).await,
    }
}

/// Poll telemetry for `window`, returning (ms since `epoch`, reported power W) per report.
/// Sequential by design: writes and observation never overlap, which keeps the write →
/// response causality in the log unambiguous. `Ok(None)` (connection ended) stops early.
async fn collect_reports(
    conn: &mut Box<dyn TrainerConnectionTrait>,
    epoch: Instant,
    window: Duration,
) -> Vec<(u128, f64)> {
    let deadline = Instant::now() + window;
    let mut reports = Vec::new();
    while Instant::now() < deadline {
        // The simulated trainer fast-forwards virtual time and can flood reports far
        // faster than wall-clock; real BLE delivers ~1–4 Hz. Cap the collection so both
        // backends behave comparably (the first reports are the latency-relevant ones).
        if reports.len() >= MAX_COLLECTED_REPORTS {
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match tokio::time::timeout(remaining.min(WRITE_PACING), conn.next_telemetry()).await {
            Ok(Ok(Some(t))) => {
                reports.push((epoch.elapsed().as_millis(), t.power_w));
                if reports.len() <= MAX_PRINTED_REPORTS {
                    print_report(epoch, t.power_w);
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(err)) => {
                eprintln!("telemetry error (continuing): {err}");
                break;
            }
            Err(_elapsed) => continue, // pacing tick; re-check the deadline
        }
    }
    if reports.len() > MAX_PRINTED_REPORTS {
        println!(
            "    … {} more reports (total {}) — full list in the caller's return value",
            reports.len() - MAX_PRINTED_REPORTS,
            reports.len()
        );
    }
    reports
}

/// Keep the printed log readable: the first reports matter (latency), the rest are noise.
const MAX_PRINTED_REPORTS: usize = 8;

/// Cap on reports collected per observation window (see `collect_reports`).
const MAX_COLLECTED_REPORTS: usize = 100;

fn print_report(epoch: Instant, power_w: f64) {
    println!(
        "    t+{:>6} ms  {:>6.1} W",
        epoch.elapsed().as_millis(),
        power_w
    );
}

fn header(title: &str) {
    println!("\n=====================================================");
    println!("== {title}");
    println!("=====================================================");
}

/// Experiment 1 — grade ramp. Proves 0% is accepted, then ramps past the documented cap,
/// then a negative grade, printing one row per step for the datasheet.
async fn experiment_ramp(conn: &mut Box<dyn TrainerConnectionTrait>) -> Result<(), String> {
    header("Experiment 1: grade ramp (0→5→10→15→16→17→20→-5%)");
    println!(
        "Documented cap: {EXPECTED_MAX_GRADE_PERCENT}% (JetBlack official; trainer_core device_quirks)."
    );
    println!("Record live: where resistance stops following (cap) and what 17%+ does");
    println!("(clamps silently / ignored / error response). FEEL the resistance.\n");
    let epoch = Instant::now();
    let steps = [0.0f64, 5.0, 10.0, 15.0, 16.0, 17.0, 20.0, -5.0];
    for step in steps {
        let params = SimParams {
            grade: GradeFraction::from_percent(step),
            ..SimParams::default()
        };
        print!(
            "[t+{:>6} ms] grade {step:>5.1}% : ",
            epoch.elapsed().as_millis()
        );
        std::io::Write::flush(&mut std::io::stdout()).ok();
        match conn.set_sim_params(&params).await {
            Ok(()) => {
                println!("write ACKed");
                tokio::time::sleep(Duration::from_millis(3000)).await;
                let n = collect_reports(conn, epoch, Duration::from_secs(2))
                    .await
                    .len();
                println!("           ({n} telemetry reports in the 2 s window)");
            }
            Err(err) => println!("write REJECTED: {err}"),
        }
        tokio::time::sleep(WRITE_PACING).await;
    }
    println!("\nDatasheet: cap = highest grade where resistance still followed; behavior");
    println!("above it (clamp/ignore/error) goes in jetblack-victory.md 'Grade cap behavior'.");
    Ok(())
}

/// Experiment 2 — param units and signs. Wind sign check (head +, tail −), then Crr and
/// Cw steps large enough to feel.
async fn experiment_units(conn: &mut Box<dyn TrainerConnectionTrait>) -> Result<(), String> {
    header("Experiment 2: wind sign + Crr/Cw steps (grade held at 0%)");
    println!("Record felt direction/magnitude after each ACK.\n");
    let epoch = Instant::now();
    let steps: [(&str, SimParams); 4] = [
        (
            "wind +2.0 m/s (spec: headwind)",
            SimParams {
                wind_speed_mps: 2.0,
                ..Default::default()
            },
        ),
        (
            "wind -2.0 m/s (spec: tailwind)",
            SimParams {
                wind_speed_mps: -2.0,
                ..Default::default()
            },
        ),
        (
            "Crr 0.012 (vs 0.005)",
            SimParams {
                crr: 0.012,
                ..Default::default()
            },
        ),
        (
            "CdA 0.60 (vs 0.32)",
            SimParams {
                cda: DragArea::from_m2(0.60),
                ..Default::default()
            },
        ),
    ];
    for (label, params) in steps {
        match conn.set_sim_params(&params).await {
            Ok(()) => println!("[t+{:>6} ms] {label}: ACKed", epoch.elapsed().as_millis()),
            Err(err) => println!(
                "[t+{:>6} ms] {label}: REJECTED: {err}",
                epoch.elapsed().as_millis()
            ),
        }
        tokio::time::sleep(WRITE_PACING).await;
    }
    println!("\nIf +wind and -wind feel identical, the sign convention or scaling differs");
    println!("from spec — datasheet 'quirks' row.");
    Ok(())
}

/// Experiment 3 — latency and smoothing: grade steps with per-report timestamps printed
/// relative to the write, so response latency and ramp shape read straight off the log.
async fn experiment_latency(conn: &mut Box<dyn TrainerConnectionTrait>) -> Result<(), String> {
    header("Experiment 3: grade step response (latency + smoothing)");
    println!("For each step note when resistance STARTS moving and whether it snaps or ramps.\n");
    for grade in [0.0f64, 8.0, 0.0, 12.0, 0.0] {
        let epoch = Instant::now();
        let params = SimParams {
            grade: GradeFraction::from_percent(grade),
            ..SimParams::default()
        };
        match conn.set_sim_params(&params).await {
            Ok(()) => {
                println!(">>> grade {grade:>4.1}% write ACKed at t=0");
                let _ = collect_reports(conn, epoch, Duration::from_secs(5)).await;
            }
            Err(err) => println!(">>> grade {grade:>4.1}% REJECTED: {err}"),
        }
        tokio::time::sleep(WRITE_PACING).await;
    }
    println!("\nLatency = first report where power moved vs the write timestamp above.");
    Ok(())
}

/// Experiment 4 — sustained scripted terrain profile, including coast segments probing
/// the coast-down power-floor flaw (GPLama 2024-10) on this firmware.
async fn experiment_sustain(conn: &mut Box<dyn TrainerConnectionTrait>) -> Result<(), String> {
    header("Experiment 4: sustained terrain profile (~10 min) + coast-down check");
    println!("Profile: 3min flat, 3min @ 8%, 2min @ -4%, 2min flat, then coast at 0%.");
    println!("Pedal throughout; at the coast segment, stop pedaling and watch the floor.\n");
    let epoch = Instant::now();
    let profile = [
        (0.0f64, 180u64, "flat threshold"),
        (8.0, 180, "climb 8%"),
        (-4.0, 120, "descent -4%"),
        (0.0, 120, "flat 2"),
    ];
    for (grade, seconds, label) in profile {
        let params = SimParams {
            grade: GradeFraction::from_percent(grade),
            ..SimParams::default()
        };
        conn.set_sim_params(&params)
            .await
            .map_err(|err| format!("sustain: grade {grade}% write failed: {err}"))?;
        println!(">>> {label}: grade {grade}% held {seconds}s (reports below)");
        let reports = collect_reports(conn, epoch, Duration::from_secs(seconds)).await;
        if let (Some(first), Some(last)) = (reports.first(), reports.last()) {
            println!(
                "    segment summary: first {:.0} W, last {:.0} W over {} reports",
                first.1,
                last.1,
                reports.len()
            );
        }
    }

    println!("\n>>> coast-down check: 0%, rider stops pedaling — power should fall to 0.");
    let reports = collect_reports(conn, epoch, Duration::from_secs(45)).await;
    if let Some(floor) = reports.iter().map(|r| r.1).reduce(f64::min) {
        println!(
            "    min reported power during coast: {floor:.0} W (a ~25 W floor = the GPLama flaw; record firmware!)"
        );
    }
    Ok(())
}

/// Experiment 5 — trainer-reported power vs the app's physics model at several grades.
/// Ride each grade at steady cadence; the spike prints the model's balance speed and the
/// average trainer-reported power over a settled window.
async fn experiment_telemetry(conn: &mut Box<dyn TrainerConnectionTrait>) -> Result<(), String> {
    header("Experiment 5: trainer power vs powerForSpeed model");
    println!(
        "Model: P = (m·g·grade + Crr·m·g + ½ρ·CdA·v²)·v  (m={:.0} kg, Crr={:.3}, CdA={:.2})",
        RIDER.total_mass_kg, RIDER.crr, RIDER.cda_m2
    );
    println!("Ride each grade ~20 s at steady cadence; note trainer-reported W vs model W.\n");
    let epoch = Instant::now();
    println!("| grade % | model speed @150 W (m/s) | avg trainer W |");
    for grade_pct in [0.0f64, 4.0, 8.0, 12.0] {
        let params = SimParams {
            grade: GradeFraction::from_percent(grade_pct),
            ..SimParams::default()
        };
        conn.set_sim_params(&params)
            .await
            .map_err(|err| format!("telemetry: grade {grade_pct}% write failed: {err}"))?;
        // Settle, then average a short window of trainer reports.
        tokio::time::sleep(Duration::from_secs(8)).await;
        let reports = collect_reports(conn, epoch, Duration::from_secs(12)).await;
        let avg_power = if reports.is_empty() {
            f64::NAN
        } else {
            reports.iter().map(|r| r.1).sum::<f64>() / reports.len() as f64
        };
        let model_speed = solve_speed_for_power(150.0, grade_pct / 100.0);
        println!(
            "| {:>7.1} | {:>24.2} | {:>13.1} |",
            grade_pct, model_speed, avg_power
        );
        tokio::time::sleep(WRITE_PACING).await;
    }
    println!("\nDivergence per grade feeds Slice 9's measured-vs-simulated separation design.");
    Ok(())
}

/// Spike-local power-model solver: the speed at which a rider producing `power_w` balances
/// the given grade. Bisection over the physically sensible range; if no speed in range
/// balances, NaN propagates to the printed row on purpose — surfaced, not clamped.
fn solve_speed_for_power(power_w: f64, grade_fraction: f64) -> f64 {
    let power_at = |v: f64| -> f64 {
        let slope_force = RIDER.total_mass_kg * 9.81 * grade_fraction;
        let roll_force = RIDER.crr * RIDER.total_mass_kg * 9.81;
        let aero_force = 0.5 * AIR_DENSITY_KG_M3 * RIDER.cda_m2 * v * v;
        (slope_force + roll_force + aero_force) * v
    };
    let (mut lo, mut hi) = (0.0f64, 30.0f64);
    if power_at(hi) < power_w {
        return f64::NAN; // unachievable at this grade/power
    }
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if power_at(mid) < power_w {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

fn print_help() {
    println!(
        "ftms-spike — FTMS SIM-mode write-path experiments\n\
         \n\
         USAGE: ftms-spike [--exp NAME] [--name PREFIX] [--sim]\n\
         \n\
         --exp ramp|units|latency|sustain|telemetry   experiment to run (default: ramp)\n\
         --name PREFIX   advertised-name filter (default: {DEFAULT_NAME_FILTER})\n\
         --sim           run against the core's simulated trainer instead of BLE\n\
         \n\
         Runs the five experiments against the real trainer (BlueZ) or, with --sim,\n\
         against the simulated trainer for rehearsal/CI. Output rows paste into\n\
         docs/trainers/jetblack-victory.md's verification log."
    );
}
