use race_physics::RiderPhysicalParams;

use crate::{zone_for_effort, WorkloadZone};

/// Everything about a rider a session needs: sustainable power, physical parameters, and the
/// cadence they prefer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiderProfile {
    /// Functional Threshold Power, in watts: what every `PercentFtp` target resolves against.
    pub ftp_watts: f64,
    /// Mass, Crr, CdA and drivetrain efficiency.
    pub physical: RiderPhysicalParams,
    /// Preferred cadence, in rpm. A presentational planning input; it never enters the physics.
    pub preferred_cadence_rpm: u32,
}

impl RiderProfile {
    /// The default preferred cadence: a middling spin.
    pub const DEFAULT_PREFERRED_CADENCE_RPM: u32 = 90;

    /// A profile with the default preferred cadence.
    pub fn new(ftp_watts: f64, physical: RiderPhysicalParams) -> Self {
        Self {
            ftp_watts,
            physical,
            preferred_cadence_rpm: Self::DEFAULT_PREFERRED_CADENCE_RPM,
        }
    }

    /// The zone `power_watts` falls into, relative to this rider's FTP.
    pub fn zone_for_watts(&self, power_watts: f64) -> &'static WorkloadZone {
        zone_for_effort(if self.ftp_watts == 0.0 {
            0.0
        } else {
            power_watts / self.ftp_watts
        })
    }
}
