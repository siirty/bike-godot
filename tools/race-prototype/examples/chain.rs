//! Run the focused offline-fit experiment, preserving rejected-plan evidence.
use race_prototype::{chain, Options};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let out = args
        .first()
        .cloned()
        .unwrap_or_else(|| "chain-report.json".into());
    let matrix = args.get(1).is_some_and(|s| s == "matrix");
    let mut reports = Vec::new();
    let inputs = if matrix {
        [180.0, 250.0, 330.0]
            .into_iter()
            .flat_map(|ftp| {
                [65.0, 85.0, 110.0].into_iter().flat_map(move |mass| {
                    [1, 42, 99]
                        .into_iter()
                        .map(move |seed| Options { ftp, mass, seed })
                })
            })
            .collect()
    } else {
        vec![Options::default()]
    };
    for options in inputs {
        let report = chain::compile(options)?.report()?;
        println!(
            "{}W {}kg seed{}: {:?}",
            report.options.ftp, report.options.mass, report.options.seed, report.acceptance
        );
        for r in &report.runs {
            println!(
                "  {:?}: effort {:?}; finish {:?}",
                r.execution,
                r.steps
                    .iter()
                    .map(|s| (s.effort_share * 100.0).round())
                    .collect::<Vec<_>>(),
                r.finish_s
            );
        }
        reports.push(report);
    }
    let path = std::path::Path::new(&out);
    std::fs::write(path, serde_json::to_vec_pretty(&reports)?)?;
    if !matrix {
        let json = serde_json::to_string(&reports[0])?.replace('<', "\\u003c");
        let html = include_str!("../src/chain-viewer.html").replace("__CHAIN_DATA__", &json);
        std::fs::write(path.with_extension("html"), html)?;
    }
    Ok(())
}
