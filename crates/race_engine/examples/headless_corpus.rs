//! Runs the seeded headless corpus and prints a markdown report.
//!
//! ```sh
//! cargo run -p race_engine --features testing --example headless_corpus --release -- [first] [count]
//! ```

use race_engine::harness::{render_corpus_report, run_corpus};

fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .map(|a| a.parse().expect("seed arguments are integers"))
        .collect();
    let first = args.first().copied().unwrap_or(0);
    let count = args.get(1).copied().unwrap_or(16);
    match run_corpus(first..first + count) {
        Ok(reports) => print!("{}", render_corpus_report(&reports)),
        Err(e) => {
            eprintln!("corpus failed: {e}");
            std::process::exit(1);
        }
    }
}
