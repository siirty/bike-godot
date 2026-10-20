//! Mixed-field verification and action occupancy; no claim of calibrated race behavior.
use race_engine::harness::TacticalScenario;
fn main() {
    let count: u64 = std::env::args()
        .nth(1)
        .map_or(16, |s| s.parse().expect("integer seed count"));
    println!("| seed | finished | steps | recover | follow | chase | cruise | attack | sprint |");
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for seed in 0..count {
        let mut scenario = TacticalScenario::generate(seed).expect("valid seed");
        let (run, a) = scenario.run().expect("valid race");
        let n = run.finishers().len();
        assert_eq!(
            n,
            scenario.base.config.riders().len(),
            "seed {seed} did not finish"
        );
        println!(
            "| {seed} | {n} | {} | {} | {} | {} | {} | {} | {} |",
            run.steps, a[0], a[1], a[2], a[3], a[4], a[5]
        );
    }
}
