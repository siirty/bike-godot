//! Zwift `.zwo` parsing.
//!
//! Segment handling follows GoldenCheetah's `ZwoParser.cpp`: a flat
//! power attribute wins over a low/high pair; `<SteadyState>` with `PowerLow`/`PowerHigh` is a zone
//! band ridden at its midpoint, not a ramp; `<Cooldown>` always ramps high-to-low; `IntervalsT`'s
//! `Repeat` defaults to 1 and its off phase is optional.

use std::time::Duration;

use roxmltree::{Document, Node};

use crate::error::WorkoutImportError;
use crate::model::{IntervalRepeat, IntervalRepeatSpec, PowerTarget, WorkoutPlan, WorkoutSegment};
use crate::numparse::{parse_f64, parse_i64};

const FORMAT: &str = "zwo";

fn err(message: impl Into<String>) -> WorkoutImportError {
    WorkoutImportError::with_format(message, FORMAT)
}

/// Parses the XML text of a `.zwo` file. Powers stay unresolved [`PowerTarget::PercentFtp`]s.
pub fn parse_zwo_text(contents: &str) -> Result<WorkoutPlan, WorkoutImportError> {
    let document = Document::parse(contents).map_err(|e| err(format!("malformed XML: {e}")))?;
    let root = document.root_element();
    if root.tag_name().name() != "workout_file" {
        return Err(err(format!(
            "expected root element <workout_file>, found <{}>",
            root.tag_name().name()
        )));
    }

    let name = child(root, "name").ok_or_else(|| err("missing required <name> element"))?;
    let workout =
        child(root, "workout").ok_or_else(|| err("missing required <workout> element"))?;

    let segments = workout
        .children()
        .filter(Node::is_element)
        .map(parse_segment)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(WorkoutPlan {
        name: Some(inner_text(name).trim().to_owned()),
        author: optional_text(root, "author"),
        description: optional_text(root, "description"),
        segments,
    })
}

fn child<'a, 'i>(node: Node<'a, 'i>, tag: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == tag)
}

/// All descendant text, concatenated.
fn inner_text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(Node::is_text)
        .filter_map(|n| n.text())
        .collect()
}

fn optional_text(root: Node<'_, '_>, tag: &str) -> Option<String> {
    let text = inner_text(child(root, tag)?);
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn parse_segment(element: Node<'_, '_>) -> Result<WorkoutSegment, WorkoutImportError> {
    match element.tag_name().name() {
        "SteadyState" => {
            let duration = duration_attr(element, "Duration")?;
            let (low, high) = power_pair(element, "Power", "PowerLow", "PowerHigh")?;
            Ok(WorkoutSegment::SteadyState {
                duration,
                target_power: PowerTarget::PercentFtp((low + high) / 2.0),
                cadence: cadence(element)?,
            })
        }
        tag @ ("Warmup" | "Ramp" | "Cooldown") => {
            let duration = duration_attr(element, "Duration")?;
            let (mut low, mut high) = power_pair(element, "Power", "PowerLow", "PowerHigh")?;
            if tag == "Cooldown" && low < high {
                std::mem::swap(&mut low, &mut high);
            }
            Ok(WorkoutSegment::Ramp {
                duration,
                start_power: PowerTarget::PercentFtp(low),
                end_power: PowerTarget::PercentFtp(high),
                cadence: cadence(element)?,
            })
        }
        "IntervalsT" => parse_intervals_t(element),
        "FreeRide" | "Freeride" => Ok(WorkoutSegment::FreeRide {
            duration: duration_attr(element, "Duration")?,
            cadence: cadence(element)?,
        }),
        other => Err(err(format!("unsupported .zwo segment element <{other}>"))),
    }
}

fn parse_intervals_t(element: Node<'_, '_>) -> Result<WorkoutSegment, WorkoutImportError> {
    let raw_repeat = match element.attribute("Repeat") {
        None => 0,
        Some(_) => int_attr(element, "Repeat")?,
    };
    let repeat = if raw_repeat == 0 { 1 } else { raw_repeat };
    let repeat = u32::try_from(repeat).map_err(|_| {
        err(format!(
            "attribute \"Repeat\" on <IntervalsT> must be a positive count, found {repeat}"
        ))
    })?;

    let on_duration = duration_attr(element, "OnDuration")?;
    let (on_low, on_high) = power_pair(element, "OnPower", "PowerOnLow", "PowerOnHigh")?;

    let off_duration = match element.attribute("OffDuration") {
        None => Duration::ZERO,
        Some(_) => duration_attr(element, "OffDuration")?,
    };
    let (off_start_power, off_end_power) = if off_duration.is_zero() {
        (None, None)
    } else {
        let (low, high) = power_pair(element, "OffPower", "PowerOffLow", "PowerOffHigh")?;
        (
            Some(PowerTarget::PercentFtp(low)),
            Some(PowerTarget::PercentFtp(high)),
        )
    };

    let block = IntervalRepeat::new(IntervalRepeatSpec {
        repeat,
        on_duration,
        on_start_power: PowerTarget::PercentFtp(on_low),
        on_end_power: PowerTarget::PercentFtp(on_high),
        off_duration,
        off_start_power,
        off_end_power,
        on_cadence: cadence(element)?,
        off_cadence: int_attr_opt(element, "CadenceResting")?,
    })
    .map_err(|e| err(e.message().to_owned()))?;
    Ok(WorkoutSegment::IntervalRepeat(block))
}

fn cadence(element: Node<'_, '_>) -> Result<Option<i64>, WorkoutImportError> {
    int_attr_opt(element, "Cadence")
}

fn power_pair(
    element: Node<'_, '_>,
    flat: &str,
    low: &str,
    high: &str,
) -> Result<(f64, f64), WorkoutImportError> {
    if element.attribute(flat).is_some() {
        let value = num_attr(element, flat)?;
        return Ok((value, value));
    }
    if element.attribute(low).is_none() {
        return Err(err(format!(
            "missing required \"{flat}\" or \"{low}\" attribute on <{}>",
            element.tag_name().name()
        )));
    }
    let low_value = num_attr(element, low)?;
    let high_value = match element.attribute(high) {
        None => low_value,
        Some(_) => num_attr(element, high)?,
    };
    Ok((low_value, high_value))
}

fn duration_attr(element: Node<'_, '_>, attribute: &str) -> Result<Duration, WorkoutImportError> {
    let seconds = num_attr(element, attribute)?;
    let micros = (seconds * 1_000_000.0).round();
    if !micros.is_finite() || micros < 0.0 || micros > i64::MAX as f64 {
        return Err(err(format!(
            "attribute \"{attribute}\" on <{}> is not a usable duration: {seconds}",
            element.tag_name().name()
        )));
    }
    Ok(Duration::from_micros(micros as u64))
}

fn num_attr(element: Node<'_, '_>, attribute: &str) -> Result<f64, WorkoutImportError> {
    let tag = element.tag_name().name();
    let raw = element.attribute(attribute).ok_or_else(|| {
        err(format!(
            "missing required \"{attribute}\" attribute on <{tag}>"
        ))
    })?;
    parse_f64(raw).ok_or_else(|| {
        err(format!(
            "attribute \"{attribute}\" on <{tag}> is not a number: \"{raw}\""
        ))
    })
}

fn int_attr(element: Node<'_, '_>, attribute: &str) -> Result<i64, WorkoutImportError> {
    let tag = element.tag_name().name();
    let raw = element.attribute(attribute).ok_or_else(|| {
        err(format!(
            "missing required \"{attribute}\" attribute on <{tag}>"
        ))
    })?;
    parse_i64(raw).ok_or_else(|| {
        err(format!(
            "attribute \"{attribute}\" on <{tag}> is not an integer: \"{raw}\""
        ))
    })
}

fn int_attr_opt(element: Node<'_, '_>, attribute: &str) -> Result<Option<i64>, WorkoutImportError> {
    match element.attribute(attribute) {
        None => Ok(None),
        Some(_) => int_attr(element, attribute).map(Some),
    }
}
