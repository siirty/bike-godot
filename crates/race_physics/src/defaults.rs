/// Default values for the tunable physical parameters. Defaults, not constants: every one is
/// expected to be tuned per rider and equipment.
///
/// They live in one place so `RiderPhysicalParams`, `EnvironmentParams` and `trainer_control`'s
/// `SimulationParameters` cannot disagree. `trainer_core` keeps its own copy (separate repository)
/// and names this type as the source of truth.
pub struct PhysicsDefaults;

impl PhysicsDefaults {
    /// Coefficient of rolling resistance. Typical range 0.004–0.008 on smooth asphalt.
    pub const CRR: f64 = 0.005;
    /// Drag area (CdA), in m². Typical range 0.19–0.40 depending on position.
    pub const CDA_M2: f64 = 0.32;
    /// Drivetrain efficiency as a fraction. Typical range 0.97–0.98.
    pub const DRIVE_TRAIN_EFFICIENCY: f64 = 0.975;
    /// Air density, in kg/m³: sea level at 15 °C.
    pub const AIR_DENSITY_KG_M3: f64 = 1.225;
}
