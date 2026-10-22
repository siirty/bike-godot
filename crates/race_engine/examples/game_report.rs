//! Reproducible game-design comparisons. Findings flag weaknesses rather than certify balance.
use race_engine::{
    game_session::*,
    mixed::{MixedOptions, PLAYER},
};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Serialize)]
enum Change {
    Reference,
    Miss,
    Delay,
    Overdo,
    Conserve,
    IgnoreAll,
}
#[derive(Serialize)]
struct Block {
    block: usize,
    seconds: f64,
    band_fraction: f64,
    mean_w: f64,
    sheltered_fraction: Option<f64>,
    end_front_gap_m: Option<f64>,
}
#[derive(Serialize)]
struct Checkpoint {
    label: String,
    workout_s: f64,
    phase: Phase,
    riders: Vec<(String, f64)>,
    reserve_j: f64,
}
#[derive(Serialize)]
struct Run {
    name: String,
    change: Change,
    changed_block: Option<usize>,
    outcome: Option<Outcome>,
    completion: Completion,
    blocks: Vec<Block>,
    checkpoints: Vec<Checkpoint>,
    samples: Vec<GameSample>,
}
fn run(initial: &GameSession, name: String, change: Change, changed: Option<usize>) -> Run {
    let mut session = initial.clone();
    let mut count = [[0.0; 5]; 8];
    let mut gaps = [None; 8];
    let mut samples = vec![];
    let mut checkpoints = vec![];
    while session.phase() != Phase::Finished {
        let block = session.block();
        let target = session.target_w();
        let at = session.workout_time().as_secs_f64();
        let start = match block {
            2 => 1500.0,
            4 => 2280.0,
            5 => 2640.0,
            _ => 0.0,
        };
        let watts = match change {
            Change::Miss if Some(block) == changed => 204.0,
            Change::Delay if Some(block) == changed && at < start + 30.0 => 204.0,
            Change::Overdo if Some(block) == changed => target * 1.15,
            Change::Conserve if [1, 3, 6].contains(&block) => target * 0.95,
            Change::IgnoreAll if [2, 4, 5].contains(&block) => 204.0,
            _ => target,
        };
        let sample = session.step(watts, 1, None).unwrap();
        let c = &mut count[block];
        c[0] += 1.0;
        c[1] += f64::from(sample.in_band);
        c[2] += sample.delivered_w;
        if let Some(r) = &sample.race {
            c[3] += 1.0;
            let human = r.riders.iter().find(|r| r.id == PLAYER).unwrap();
            c[4] += f64::from(human.wheel.is_some());
            gaps[block] = Some(
                r.riders
                    .iter()
                    .map(|r| r.distance_m)
                    .max_by(f64::total_cmp)
                    .unwrap()
                    - human.distance_m,
            );
        }
        if session.active_time().as_millis().is_multiple_of(1000) {
            for (time, label) in [
                (601, "Race start +1 s"),
                (760, "Alex move checkpoint"),
                (1500, "Rowan move checkpoint"),
                (1545, "Chase response checkpoint"),
                (1680, "After chase"),
                (2640, "After tempo"),
                (2700, "After short response"),
                (3300, "Competitive deadline"),
            ] {
                if session.workout_time().as_secs() == time {
                    let riders = sample
                        .race
                        .as_ref()
                        .map(|r| {
                            let human = r.riders.iter().find(|r| r.id == PLAYER).unwrap();
                            r.riders
                                .iter()
                                .map(|r| (r.id.clone(), r.distance_m - human.distance_m))
                                .collect()
                        })
                        .unwrap_or_default();
                    checkpoints.push(Checkpoint {
                        label: label.into(),
                        workout_s: sample.workout_s,
                        phase: sample.phase,
                        riders,
                        reserve_j: sample.reserve_j,
                    });
                }
            }
            samples.push(sample);
        }
    }
    let blocks = count
        .iter()
        .enumerate()
        .map(|(i, c)| Block {
            block: i,
            seconds: c[0] * 0.05,
            band_fraction: c[1] / c[0].max(1.0),
            mean_w: c[2] / c[0].max(1.0),
            sheltered_fraction: (c[3] > 0.0).then(|| c[4] / c[3]),
            end_front_gap_m: gaps[i],
        })
        .collect();
    Run {
        name,
        change,
        changed_block: changed,
        outcome: session.outcome().cloned(),
        completion: session.completion(),
        blocks,
        checkpoints,
        samples,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("game-session-output"), PathBuf::from);
    let initial = GameSession::new(MixedOptions::default())?;
    let mut runs = vec![run(&initial, "Reference".into(), Change::Reference, None)];
    for block in [2, 4, 5] {
        for change in [Change::Miss, Change::Delay, Change::Overdo] {
            runs.push(run(
                &initial,
                format!("{change:?} block {}", block + 1),
                change,
                Some(block),
            ));
        }
    }
    runs.push(run(
        &initial,
        "Conserve 5% in endurance".into(),
        Change::Conserve,
        None,
    ));
    runs.push(run(
        &initial,
        "Ignore all contributions".into(),
        Change::IgnoreAll,
        None,
    ));
    let mut findings = vec![];
    if runs[0].outcome.as_ref().unwrap().verdict() != Verdict::Success {
        findings.push("Reference loses the objective: reject this nominal game fit.".to_owned());
    }
    for run in &runs[1..] {
        let success = run.outcome.as_ref().unwrap().verdict() == Verdict::Success;
        if matches!(run.change, Change::Miss | Change::IgnoreAll) && success {
            findings.push(format!("{} still succeeds: the objective does not establish that this contribution is necessary.",run.name));
        }
        if matches!(run.change, Change::Conserve) && !success {
            findings.push("Conserving 5% loses the objective: the endurance opportunity is sensitive to small under-effort.".to_owned());
        }
        if matches!(run.change, Change::Overdo) && success {
            findings.push(format!("{} succeeds with extra work; compare its gaps before claiming that prescribed effort is the strongest game move.",run.name));
        }
        if matches!(run.change, Change::Overdo)
            && !success
            && runs[0].outcome.as_ref().unwrap().verdict() == Verdict::Success
        {
            findings.push(format!("{} loses despite extra work: inspect fatigue and frozen pacing for brittle or counterintuitive game consequences.",run.name));
        }
    }
    let report = json!({"version":2,"software_only":true,"model_status":"Provisional game world; not empirically validated","objective":"At least one Ember rider, including the player, in the leading group at the 45-minute competitive deadline","runs":runs,"findings":findings});
    std::fs::create_dir_all(&out)?;
    let data = serde_json::to_string(&report)?;
    std::fs::write(out.join("game-report.json"), &data)?;
    // Presentation-only projection keeps the shareable viewer compact. Exact evidence stays in JSON.
    let mut viewer = report.clone();
    for run in viewer["runs"].as_array_mut().unwrap() {
        for sample in run["samples"].as_array_mut().unwrap() {
            sample.as_object_mut().unwrap().remove("in_band");
            if let Some(race) = sample["race"].as_object_mut() {
                race.retain(|key, _| ["observation", "scene", "riders"].contains(&key.as_str()));
                race["observation"]
                    .as_object_mut()
                    .unwrap()
                    .retain(|key, _| ["topology", "pursuit"].contains(&key.as_str()));
                for rider in race["riders"].as_array_mut().unwrap() {
                    rider.as_object_mut().unwrap().retain(|key, _| {
                        ["id", "distance_m", "speed_ms", "wheel", "reserve_j"]
                            .contains(&key.as_str())
                    });
                    for key in ["distance_m", "speed_ms", "reserve_j"] {
                        let value = rider[key].as_f64().unwrap();
                        rider[key] = json!((value * 100.0).round() / 100.0);
                    }
                }
            }
        }
    }
    let viewer_data = serde_json::to_string(&viewer)?;
    std::fs::write(
        out.join("game-report.html"),
        include_str!("game_viewer.html").replace("__DATA__", &viewer_data.replace('<', "\\u003c")),
    )?;
    let mut markdown=String::from("# Game session comparison\n\nProvisional game model. Input variants are software execution tests, not human recognition tests.\n\n| Run | Result | Front gap at deadline | Post-chase shelter |\n|---|---|---:|---:|\n");
    for run in &runs {
        markdown += &format!(
            "| {} | {:?} | {:.1} m | {:.1}% |\n",
            run.name,
            run.outcome.as_ref().unwrap().verdict(),
            run.outcome
                .as_ref()
                .unwrap()
                .player_front_gap_m()
                .unwrap_or(0.0),
            run.blocks[3].sheltered_fraction.unwrap_or(0.0) * 100.0
        );
    }
    markdown += "\n## Design findings\n\n";
    for finding in findings {
        markdown += &format!("- {finding}\n");
    }
    std::fs::write(out.join("game-report.md"), &markdown)?;
    print!("{markdown}");
    Ok(())
}
