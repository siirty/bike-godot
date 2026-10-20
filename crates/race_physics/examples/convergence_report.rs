//! Prints the fixed-timestep comparison table for this crate's README.
//!
//! ```sh
//! cargo run -p race_physics --features testing --example convergence_report --release
//! ```

fn main() {
    print!(
        "{}",
        race_physics::convergence::render_convergence_report().expect("corpus is well-formed")
    );
}
