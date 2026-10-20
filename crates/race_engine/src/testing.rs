//! Tolerance-aware comparison of simulation values, for tests only.
//!
//! Behind the `testing` feature on purpose: tolerance is a property of a test, not of the domain.
//! Domain `==` and same-runtime snapshot round trips stay exact. This exists for comparing races
//! stepped on different platforms, whose doubles may differ in the last bits.
//!
//! Everything discrete is compared exactly even here: ids, enums, durations, lengths and order,
//! lateral slots, RNG state, and event variants. Only `f64`s get
//! `|a - b| <= abs + rel * max(|a|, |b|)`. Every method returns human-readable differences
//! prefixed with a field path; empty means equivalent.

use std::fmt::Debug;

use serde_json::Value;

use crate::{
    BasicDraftingConfig, RaceEvent, RaceInputFrame, RaceSimulationConfig, RaceState,
    RaceStepResult, RiderDefinition, SimulationSnapshot, SimulationTerrain,
};

/// The comparison and its tolerances.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeterministicComparison {
    /// Absolute term; dominates near zero.
    pub absolute_tolerance: f64,
    /// Relative term; dominates for large magnitudes.
    pub relative_tolerance: f64,
}

impl Default for DeterministicComparison {
    fn default() -> Self {
        Self {
            absolute_tolerance: Self::DEFAULT_ABSOLUTE_TOLERANCE,
            relative_tolerance: Self::DEFAULT_RELATIVE_TOLERANCE,
        }
    }
}

type Diffs = Vec<String>;

impl DeterministicComparison {
    /// A nanometre (per second): far below anything the simulation means, far above f64 noise.
    pub const DEFAULT_ABSOLUTE_TOLERANCE: f64 = 1e-9;
    /// About seven decimal digits of slack in a double's sixteen.
    pub const DEFAULT_RELATIVE_TOLERANCE: f64 = 1e-9;

    /// Bit-exact doubles: the setting for a same-runtime round trip.
    pub const fn exact() -> Self {
        Self {
            absolute_tolerance: 0.0,
            relative_tolerance: 0.0,
        }
    }

    /// Whether `a` and `b` are within tolerance. NaN never matches, same-signed infinities do.
    pub fn doubles_match(&self, a: f64, b: f64) -> bool {
        if a.is_nan() || b.is_nan() {
            return false;
        }
        if a == b {
            return true;
        }
        if a.is_infinite() || b.is_infinite() {
            return false;
        }
        (a - b).abs() <= self.absolute_tolerance + self.relative_tolerance * a.abs().max(b.abs())
    }

    /// Config and state.
    pub fn compare_snapshots(
        &self,
        a: &SimulationSnapshot,
        b: &SimulationSnapshot,
        path: &str,
    ) -> Diffs {
        let mut d = self.compare_configs(a.config(), b.config(), &format!("{path}.config"));
        d.extend(self.compare_states(a.state(), b.state(), &format!("{path}.state")));
        d
    }

    /// Two configurations.
    pub fn compare_configs(
        &self,
        a: &RaceSimulationConfig,
        b: &RaceSimulationConfig,
        path: &str,
    ) -> Diffs {
        let mut d = Diffs::new();
        exact(
            &mut d,
            &a.fixed_step(),
            &b.fixed_step(),
            &format!("{path}.fixedStep"),
        );
        exact(
            &mut d,
            &a.lateral_slot_count(),
            &b.lateral_slot_count(),
            &format!("{path}.lateralSlotCount"),
        );
        self.approx(
            &mut d,
            a.air_density_kg_m3(),
            b.air_density_kg_m3(),
            &format!("{path}.airDensityKgM3"),
        );
        self.approx(
            &mut d,
            a.headwind_ms(),
            b.headwind_ms(),
            &format!("{path}.headwindMs"),
        );
        self.approx(
            &mut d,
            a.dynamics().max_propulsive_force_n(),
            b.dynamics().max_propulsive_force_n(),
            &format!("{path}.dynamics.maxPropulsiveForceN"),
        );
        d.extend(self.compare_drafting(a.drafting(), b.drafting(), &format!("{path}.drafting")));
        d.extend(self.compare_terrains(a.terrain(), b.terrain(), &format!("{path}.terrain")));
        d.extend(lists(
            a.riders(),
            b.riders(),
            &format!("{path}.riders"),
            |x, y, p| self.compare_rider_definitions(x, y, p),
        ));
        d
    }

    /// Two terrain profiles.
    pub fn compare_terrains(
        &self,
        a: &SimulationTerrain,
        b: &SimulationTerrain,
        path: &str,
    ) -> Diffs {
        let mut d = Diffs::new();
        exact(
            &mut d,
            &a.interpolation(),
            &b.interpolation(),
            &format!("{path}.interpolation"),
        );
        exact(
            &mut d,
            &a.beyond_end(),
            &b.beyond_end(),
            &format!("{path}.beyondEnd"),
        );
        match (a.explicit_length_m(), b.explicit_length_m()) {
            (Some(x), Some(y)) => self.approx(&mut d, x, y, &format!("{path}.lengthM")),
            (x, y) => exact(&mut d, &x, &y, &format!("{path}.lengthM")),
        }
        d.extend(lists(
            a.samples(),
            b.samples(),
            &format!("{path}.samples"),
            |x, y, p| {
                let mut d = Diffs::new();
                self.approx(
                    &mut d,
                    x.distance_m(),
                    y.distance_m(),
                    &format!("{p}.distanceM"),
                );
                self.approx(
                    &mut d,
                    x.grade_fraction(),
                    y.grade_fraction(),
                    &format!("{p}.gradeFraction"),
                );
                d
            },
        ));
        d
    }

    /// Two race states, including the RNG position (exact: PCG32 is integer arithmetic).
    pub fn compare_states(&self, a: &RaceState, b: &RaceState, path: &str) -> Diffs {
        let mut d = Diffs::new();
        exact(
            &mut d,
            &a.simulation_time(),
            &b.simulation_time(),
            &format!("{path}.simulationTime"),
        );
        exact(
            &mut d,
            &a.random_state(),
            &b.random_state(),
            &format!("{path}.randomState"),
        );
        d.extend(lists(
            a.riders(),
            b.riders(),
            &format!("{path}.riders"),
            |x, y, p| {
                let mut d = Diffs::new();
                exact(&mut d, x.id(), y.id(), &format!("{p}.id"));
                self.approx(
                    &mut d,
                    x.distance_m(),
                    y.distance_m(),
                    &format!("{p}.distanceM"),
                );
                self.approx(&mut d, x.speed_ms(), y.speed_ms(), &format!("{p}.speedMs"));
                exact(
                    &mut d,
                    &x.lateral_slot(),
                    &y.lateral_slot(),
                    &format!("{p}.lateralSlot"),
                );
                exact(
                    &mut d,
                    &x.draft_target_id(),
                    &y.draft_target_id(),
                    &format!("{p}.draftTargetId"),
                );
                d
            },
        ));
        d
    }

    /// Two input frames.
    pub fn compare_input_frames(
        &self,
        a: &RaceInputFrame,
        b: &RaceInputFrame,
        path: &str,
    ) -> Diffs {
        lists(
            a.inputs(),
            b.inputs(),
            &format!("{path}.inputs"),
            |x, y, p| {
                let mut d = Diffs::new();
                exact(&mut d, x.id(), y.id(), &format!("{p}.id"));
                self.approx(
                    &mut d,
                    x.power_watts(),
                    y.power_watts(),
                    &format!("{p}.powerWatts"),
                );
                self.approx(
                    &mut d,
                    x.braking_force_n(),
                    y.braking_force_n(),
                    &format!("{p}.brakingForceN"),
                );
                exact(
                    &mut d,
                    &x.lateral_slot(),
                    &y.lateral_slot(),
                    &format!("{p}.lateralSlot"),
                );
                exact(
                    &mut d,
                    &x.draft_target_id(),
                    &y.draft_target_id(),
                    &format!("{p}.draftTargetId"),
                );
                d
            },
        )
    }

    /// Two event lists: variant, order, time and riders, all exact.
    pub fn compare_events(&self, a: &[RaceEvent], b: &[RaceEvent], path: &str) -> Diffs {
        lists(a, b, path, |x, y, p| {
            let (
                RaceEvent::Reference {
                    simulation_time: tx,
                    involved_rider_ids: rx,
                },
                RaceEvent::Reference {
                    simulation_time: ty,
                    involved_rider_ids: ry,
                },
            ) = (x, y);
            let mut d = Diffs::new();
            exact(&mut d, tx, ty, &format!("{p}.simulationTime"));
            d.extend(lists(
                rx,
                ry,
                &format!("{p}.involvedRiderIds"),
                |i, j, q| {
                    let mut d = Diffs::new();
                    exact(&mut d, i, j, q);
                    d
                },
            ));
            d
        })
    }

    /// Two step results.
    pub fn compare_step_results(
        &self,
        a: &RaceStepResult,
        b: &RaceStepResult,
        path: &str,
    ) -> Diffs {
        let mut d = self.compare_states(&a.next_state, &b.next_state, &format!("{path}.nextState"));
        d.extend(self.compare_events(&a.events, &b.events, &format!("{path}.events")));
        d
    }

    /// Physical parameters go through `race_physics`' own serde mapping, so a parameter added
    /// there is compared here without anyone remembering to.
    fn compare_rider_definitions(
        &self,
        a: &RiderDefinition,
        b: &RiderDefinition,
        path: &str,
    ) -> Diffs {
        let mut d = Diffs::new();
        exact(&mut d, a.id(), b.id(), &format!("{path}.id"));
        let to_map = |r: &RiderDefinition| match serde_json::to_value(r.physical()) {
            Ok(Value::Object(m)) => m,
            other => panic!("RiderPhysicalParams must serialize to an object, got {other:?}"),
        };
        let (ma, mb) = (to_map(a), to_map(b));
        let mut keys: Vec<&String> = ma.keys().chain(mb.keys()).collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let p = format!("{path}.physical.{key}");
            match (
                ma.get(key).and_then(Value::as_f64),
                mb.get(key).and_then(Value::as_f64),
            ) {
                (Some(x), Some(y)) => self.approx(&mut d, x, y, &p),
                _ => exact(&mut d, &ma.get(key), &mb.get(key), &p),
            }
        }
        d
    }

    fn compare_drafting(
        &self,
        a: Option<&BasicDraftingConfig>,
        b: Option<&BasicDraftingConfig>,
        path: &str,
    ) -> Diffs {
        let (Some(a), Some(b)) = (a, b) else {
            let mut d = Diffs::new();
            exact(
                &mut d,
                &a.is_some(),
                &b.is_some(),
                &format!("{path} present"),
            );
            return d;
        };
        let mut d = Diffs::new();
        self.approx(
            &mut d,
            a.max_gap_m(),
            b.max_gap_m(),
            &format!("{path}.maxGapM"),
        );
        d.extend(lists(
            a.minimum_drag_multiplier_by_slot_offset(),
            b.minimum_drag_multiplier_by_slot_offset(),
            &format!("{path}.minimumDragMultiplierBySlotOffset"),
            |x, y, p| {
                let mut d = Diffs::new();
                self.approx(&mut d, *x, *y, p);
                d
            },
        ));
        d
    }

    fn approx(&self, out: &mut Diffs, a: f64, b: f64, path: &str) {
        if !self.doubles_match(a, b) {
            out.push(format!("{path}: {a:?} != {b:?} (outside tolerance)"));
        }
    }
}

fn exact<T: PartialEq + Debug + ?Sized>(out: &mut Diffs, a: &T, b: &T, path: &str) {
    if a != b {
        out.push(format!("{path}: {a:?} != {b:?}"));
    }
}

/// Element-wise; a length mismatch is reported alone rather than as a wall of misaligned pairs.
fn lists<T>(a: &[T], b: &[T], path: &str, compare: impl Fn(&T, &T, &str) -> Diffs) -> Diffs {
    if a.len() != b.len() {
        return vec![format!("{path}: length {} != {}", a.len(), b.len())];
    }
    a.iter()
        .zip(b)
        .enumerate()
        .flat_map(|(i, (x, y))| compare(x, y, &format!("{path}[{i}]")))
        .collect()
}
