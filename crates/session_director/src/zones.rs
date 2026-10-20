/// An opaque 0–255 RGB triple; the presentation layer maps it to its own colour type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

/// A workload zone: an effort band (power / FTP) and the colour that depicts it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkloadZone {
    /// Inclusive lower bound, as a fraction of FTP.
    pub effort_min: f64,
    /// Exclusive upper bound, if any.
    pub effort_max: Option<f64>,
    /// Short name, such as `Z3`.
    pub name: &'static str,
    /// Human label of the FTP range.
    pub range_label: &'static str,
    /// The depicting colour.
    pub color: Rgb,
}

impl WorkloadZone {
    /// `name · range`, such as `Z3 · 78–96%`.
    pub fn display_label(&self) -> String {
        format!("{} · {}", self.name, self.range_label)
    }
}

/// The five zones, ascending, blue → green → yellow → orange → red.
pub const ALL_WORKLOAD_ZONES: [WorkloadZone; 5] = [
    WorkloadZone {
        effort_min: 0.0,
        effort_max: Some(0.55),
        name: "Z1",
        range_label: "<55% FTP",
        color: Rgb {
            r: 76,
            g: 134,
            b: 198,
        },
    },
    WorkloadZone {
        effort_min: 0.55,
        effort_max: Some(0.78),
        name: "Z2",
        range_label: "55–78%",
        color: Rgb {
            r: 70,
            g: 161,
            b: 93,
        },
    },
    WorkloadZone {
        effort_min: 0.78,
        effort_max: Some(0.96),
        name: "Z3",
        range_label: "78–96%",
        color: Rgb {
            r: 239,
            g: 194,
            b: 0,
        },
    },
    WorkloadZone {
        effort_min: 0.96,
        effort_max: Some(1.12),
        name: "Z4",
        range_label: "96–112%",
        color: Rgb {
            r: 229,
            g: 106,
            b: 26,
        },
    },
    WorkloadZone {
        effort_min: 1.12,
        effort_max: None,
        name: "Z5",
        range_label: "≥112%",
        color: Rgb {
            r: 199,
            g: 42,
            b: 42,
        },
    },
];

/// The zone for `effort` (power / FTP). Lower bounds are inclusive, so a boundary value belongs
/// to the zone it starts.
pub fn zone_for_effort(effort: f64) -> &'static WorkloadZone {
    let z = &ALL_WORKLOAD_ZONES;
    if effort >= z[4].effort_min {
        &z[4]
    } else if effort >= z[3].effort_min {
        &z[3]
    } else if effort >= z[2].effort_min {
        &z[2]
    } else if effort >= z[1].effort_min {
        &z[1]
    } else {
        &z[0]
    }
}
