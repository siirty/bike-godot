use std::fmt::Write as _;
use std::time::Duration;

use super::corpus::{convergence_corpus, CANDIDATE_STEPS, REFERENCE_STEP};
use super::harness::{compare_scenario, dart_reduce_max, ScenarioError, COMPARISON_GRID};
use super::scenario::ConvergenceScenario;
use crate::{LongitudinalDynamicsConfig, PhysicsError};

/// Runs the corpus at every candidate step and renders the README comparison table, identical to
/// the prototype's `renderConvergenceReport()`.
pub fn render_convergence_report() -> Result<String, PhysicsError> {
    render(
        &convergence_corpus(),
        &CANDIDATE_STEPS,
        REFERENCE_STEP,
        &LongitudinalDynamicsConfig::default(),
    )
}

fn render(
    corpus: &[ConvergenceScenario],
    candidates: &[Duration],
    reference: Duration,
    config: &LongitudinalDynamicsConfig,
) -> Result<String, PhysicsError> {
    let mut errors: Vec<Vec<ScenarioError>> = Vec::new();
    for scenario in corpus {
        errors.push(
            candidates
                .iter()
                .map(|&step| compare_scenario(scenario, step, reference, config, COMPARISON_GRID))
                .collect::<Result<_, _>>()?,
        );
    }

    let labels: Vec<String> = candidates.iter().map(|s| step_label(*s)).collect();
    let header = format!(
        "| scenario | {} |\n|---|{}|\n",
        labels.join(" | "),
        vec!["---:"; candidates.len()].join("|")
    );
    let row = |name: &str, cells: Vec<String>| format!("| `{name}` | {} |\n", cells.join(" | "));

    let mut out = String::new();
    writeln!(
        out,
        "Maximum absolute error against a {} reference of the same integrator, sampled every {} \
         ms.",
        step_label(reference),
        COMPARISON_GRID.as_millis()
    )
    .unwrap();
    out.push('\n');
    out.push_str("**Speed error (m/s)**\n\n");
    out.push_str(&header);
    for (scenario, errs) in corpus.iter().zip(&errors) {
        out.push_str(&row(
            scenario.name,
            errs.iter().map(|e| fmt(e.max_speed_error_ms())).collect(),
        ));
    }
    out.push('\n');
    out.push_str("**Distance error (m)**\n\n");
    out.push_str(&header);
    for (scenario, errs) in corpus.iter().zip(&errors) {
        out.push_str(&row(
            scenario.name,
            errs.iter().map(|e| fmt(e.max_distance_error_m())).collect(),
        ));
    }
    out.push('\n');
    out.push_str("**Error at the final sample (speed m/s / distance m)**\n\n");
    out.push_str(&header);
    for (scenario, errs) in corpus.iter().zip(&errors) {
        out.push_str(&row(
            scenario.name,
            errs.iter()
                .map(|e| {
                    let speed = dart_reduce_max(e.tracks.iter().map(|t| t.final_speed_error_ms));
                    let distance =
                        dart_reduce_max(e.tracks.iter().map(|t| t.final_distance_error_m));
                    format!("{} / {}", fmt(speed), fmt(distance))
                })
                .collect(),
        ));
    }

    let stopping: Vec<usize> = (0..corpus.len())
        .filter(|&i| {
            errors[i]
                .iter()
                .all(|e| e.tracks.iter().any(|t| t.stopping_time_error.is_some()))
        })
        .collect();
    if !stopping.is_empty() {
        out.push('\n');
        out.push_str("**Stopping error (time / distance)**\n\n");
        out.push_str(&header);
        for &i in &stopping {
            out.push_str(&row(
                corpus[i].name,
                errors[i]
                    .iter()
                    .map(|e| {
                        let track = e
                            .tracks
                            .iter()
                            .find(|t| t.stopping_time_error.is_some())
                            .expect("filtered above");
                        format!(
                            "{} ms / {} m",
                            track.stopping_time_error.unwrap().as_millis(),
                            fmt(track.stopping_distance_error_m.unwrap())
                        )
                    })
                    .collect(),
            ));
        }
    }

    let paired: Vec<usize> = (0..corpus.len())
        .filter(|&i| errors[i].iter().all(|e| e.max_gap_error_m.is_some()))
        .collect();
    if !paired.is_empty() {
        out.push('\n');
        out.push_str("**Rider-to-rider gap error (m)**\n\n");
        out.push_str(&header);
        for &i in &paired {
            out.push_str(&row(
                corpus[i].name,
                errors[i]
                    .iter()
                    .map(|e| fmt(e.max_gap_error_m.unwrap()))
                    .collect(),
            ));
        }
    }

    out.push('\n');
    out.push_str("**Worst case over the whole corpus**\n\n");
    out.push_str(&header);
    let worst = |f: &dyn Fn(&ScenarioError) -> f64| -> Vec<String> {
        (0..candidates.len())
            .map(|s| fmt(dart_reduce_max(errors.iter().map(|errs| f(&errs[s])))))
            .collect()
    };
    out.push_str(&format!(
        "| speed (m/s) | {} |\n",
        worst(&|e| e.max_speed_error_ms()).join(" | ")
    ));
    out.push_str(&format!(
        "| distance (m) | {} |\n",
        worst(&|e| e.max_distance_error_m()).join(" | ")
    ));
    out.push_str(&format!(
        "| gap (m) | {} |\n",
        worst(&|e| e.max_gap_error_m.unwrap_or(0.0)).join(" | ")
    ));
    Ok(out)
}

fn step_label(step: Duration) -> String {
    format!("{} ms", step.as_millis())
}

fn fmt(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    dart_to_string_as_precision(value, 3)
}

/// Dart's (and JavaScript's) `toStringAsPrecision`: `precision` significant digits, in
/// exponential notation when the exponent is below −6 or at least `precision`.
///
/// Rounding follows Rust's exact decimal expansion. That only differs from the prototype on an
/// exact binary tie at the last digit, which no measured error in the corpus is.
pub fn dart_to_string_as_precision(value: f64, precision: usize) -> String {
    assert!((1..=21).contains(&precision), "precision must be in 1..=21");
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    let sign = if value.is_sign_negative() && value != 0.0 {
        "-"
    } else {
        ""
    };
    let scientific = format!("{:.*e}", precision - 1, value.abs());
    let (mantissa, exponent) = scientific.split_once('e').expect("{:e} always has an e");
    let exponent: i32 = exponent.parse().expect("integer exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();

    if exponent < -6 || exponent >= precision as i32 {
        let mut out = format!("{sign}{}", &digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let exp_sign = if exponent < 0 { '-' } else { '+' };
        return format!("{out}e{exp_sign}{}", exponent.abs());
    }
    if exponent >= 0 {
        let point = exponent as usize + 1;
        if point == digits.len() {
            format!("{sign}{digits}")
        } else {
            format!("{sign}{}.{}", &digits[..point], &digits[point..])
        }
    } else {
        format!("{sign}0.{}{digits}", "0".repeat((-exponent - 1) as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::dart_to_string_as_precision as p;

    #[test]
    fn matches_dart_to_string_as_precision() {
        // Expected strings are Dart's `toStringAsPrecision(3)` on the same inputs.
        assert_eq!(p(0.0123456, 3), "0.0123");
        assert_eq!(p(1.0, 3), "1.00");
        assert_eq!(p(123.456, 3), "123");
        assert_eq!(p(1234.5, 3), "1.23e+3");
        assert_eq!(p(9.996, 3), "10.0");
        assert_eq!(p(0.000001234, 3), "0.00000123");
        assert_eq!(p(1.234e-7, 3), "1.23e-7");
        assert_eq!(p(-2.5, 3), "-2.50");
        assert_eq!(p(999.6, 3), "1.00e+3");
    }
}
