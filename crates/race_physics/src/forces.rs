//! The three resistive forces, each an individually testable pure function. Forward is positive.

/// Standard gravitational acceleration, in m/s².
pub const GRAVITY_ACCELERATION_MS2: f64 = 9.80665;

/// The slope component of gravity, in newtons: `m·g·sin(atan(grade))`. Negative on a descent.
pub fn gravity_force_n(mass_kg: f64, grade_fraction: f64) -> f64 {
    let theta = grade_fraction.atan();
    mass_kg * GRAVITY_ACCELERATION_MS2 * theta.sin()
}

/// Rolling resistance, in newtons: `m·g·Crr·cos(atan(grade))`. Always resists.
pub fn rolling_resistance_force_n(mass_kg: f64, grade_fraction: f64, crr: f64) -> f64 {
    let theta = grade_fraction.atan();
    mass_kg * GRAVITY_ACCELERATION_MS2 * crr * theta.cos()
}

/// Aerodynamic drag, in newtons: `0.5·ρ·CdA·|v+w|·(v+w)`, with `w` the headwind (negative is a
/// tailwind). Signed, so a tailwind faster than the rider assists.
pub fn aero_drag_force_n(
    air_density_kg_m3: f64,
    cda_m2: f64,
    speed_ms: f64,
    headwind_ms: f64,
) -> f64 {
    let effective_airspeed_ms = speed_ms + headwind_ms;
    0.5 * air_density_kg_m3 * cda_m2 * effective_airspeed_ms.abs() * effective_airspeed_ms
}
