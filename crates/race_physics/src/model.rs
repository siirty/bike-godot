use serde::{Deserialize, Serialize};

use crate::PhysicsDefaults;

/// The rider + bike's physical characteristics, fixed for a ride.
///
/// This crate owns the JSON shape (`totalMassKg`, `crr`, `cdaM2`, `driveTrainEfficiency`, in that
/// order). `race_engine`'s snapshot codec goes through this mapping rather than naming the fields
/// again, so a field added here reaches saved races.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RiderPhysicalParams {
    /// Combined rider + bike mass, in kilograms.
    pub total_mass_kg: f64,
    /// Coefficient of rolling resistance.
    #[serde(default = "default_crr")]
    pub crr: f64,
    /// Drag area (CdA), in m².
    #[serde(default = "default_cda")]
    pub cda_m2: f64,
    /// Drivetrain efficiency, as a fraction.
    #[serde(default = "default_efficiency")]
    pub drive_train_efficiency: f64,
}

fn default_crr() -> f64 {
    PhysicsDefaults::CRR
}
fn default_cda() -> f64 {
    PhysicsDefaults::CDA_M2
}
fn default_efficiency() -> f64 {
    PhysicsDefaults::DRIVE_TRAIN_EFFICIENCY
}

impl RiderPhysicalParams {
    /// A rider of `total_mass_kg` on default equipment.
    pub const fn new(total_mass_kg: f64) -> Self {
        Self {
            total_mass_kg,
            crr: PhysicsDefaults::CRR,
            cda_m2: PhysicsDefaults::CDA_M2,
            drive_train_efficiency: PhysicsDefaults::DRIVE_TRAIN_EFFICIENCY,
        }
    }
}

/// The environment a rider moves through at an instant. The same quantities FTMS SIM mode sends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvironmentParams {
    /// Terrain grade as rise/run (0.06 is a 6 % climb; negative descends).
    pub grade_fraction: f64,
    /// Air density, in kg/m³.
    pub air_density_kg_m3: f64,
    /// Headwind, in m/s. Negative is a tailwind.
    pub headwind_ms: f64,
}

impl Default for EnvironmentParams {
    fn default() -> Self {
        Self {
            grade_fraction: 0.0,
            air_density_kg_m3: PhysicsDefaults::AIR_DENSITY_KG_M3,
            headwind_ms: 0.0,
        }
    }
}

impl EnvironmentParams {
    /// Still air at sea level on `grade_fraction`.
    pub fn with_grade(grade_fraction: f64) -> Self {
        Self {
            grade_fraction,
            ..Self::default()
        }
    }
}
