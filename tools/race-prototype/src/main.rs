//! Generate the self-contained playback report and machine-readable experiment exports.
use race_prototype::{compile, Options};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = Options::default();
    let mut output = PathBuf::from("race-prototype-output");
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let value = args
            .get(i + 1)
            .ok_or("each argument needs a value (--out, --ftp, --mass, --seed)")?;
        match args[i].as_str() {
            "--out" => output = value.into(),
            "--ftp" => options.ftp = value.parse()?,
            "--mass" => options.mass = value.parse()?,
            "--seed" => options.seed = value.parse()?,
            _ => return Err(format!("unknown option {}", args[i]).into()),
        }
        i += 2;
    }
    let plan = compile(options)?;
    let report = plan.report()?;
    std::fs::create_dir_all(&output)?;
    let json = serde_json::to_string(&report)?;
    std::fs::write(output.join("report.json"), &json)?;
    // Escape the HTML-sensitive character even if future fixtures contain an authored </script>.
    let html =
        include_str!("viewer.html").replace("__REPORT_DATA__", &json.replace('<', "\\u003c"));
    std::fs::write(output.join("race-prototype.html"), html)?;
    std::fs::write(
        output.join("frozen-opponent-script.json"),
        serde_json::to_vec(&plan.script)?,
    )?;
    let mut csv=String::from("mode,time_s,phase,target_w,delivered_w,mechanical_demand_w,pacer_gap_m,rival_gap_m,reserve_fraction\n");
    for run in &report.runs {
        for s in &run.samples {
            csv.push_str(&format!(
                "{:?},{},{},{},{},{},{},{},{}\n",
                run.mode,
                s.t,
                s.phase,
                s.target,
                s.power,
                s.demand,
                s.gaps[2],
                s.gaps[3],
                s.reserves[0]
            ));
        }
        println!(
            "{:?}: workload {}/{}; contention proxy {}/{} applicable; recognition/result review pending; legacy formation {}/{}; finish {:?}",
            run.mode,
            run.learning_audits.iter().filter(|a| a.workout == race_prototype::learning::Verdict::MeetsDiagnostic).count(),
            run.learning_audits.len(),
            run.learning_audits.iter().filter(|a| a.continuity == race_prototype::learning::Verdict::MeetsDiagnostic).count(),
            run.learning_audits.iter().filter(|a| a.continuity != race_prototype::learning::Verdict::NotApplicable).count(),
            run.outcomes.iter().filter(|o| o.passed).count(),
            run.outcomes.len(),
            run.finish_s
        );
    }
    std::fs::write(output.join("samples.csv"), csv)?;
    println!("Open {}", output.join("race-prototype.html").display());
    Ok(())
}
