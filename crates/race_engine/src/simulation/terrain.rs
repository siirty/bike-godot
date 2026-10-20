use session_director::TerrainSource;

use crate::canonical_json::format_number;
use crate::simulation::error::{RaceSimulationError, Result, SimulationValidationCode as C};
use crate::simulation::validate::{check_finite, check_finite_non_negative, check_finite_positive};

/// How grade behaves between samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainInterpolation {
    /// Hold each sample until the next (a block profile).
    Step,
    /// Ramp linearly between samples (a decimated road).
    Linear,
}

impl TerrainInterpolation {
    /// Every value, in declaration order.
    pub const ALL: [Self; 2] = [Self::Step, Self::Linear];

    /// The snapshot name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Linear => "linear",
        }
    }

    /// The value named `name`, if this build has it.
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.name() == name)
    }
}

/// What grade is past the end of the course.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BeyondEndBehavior {
    /// Keep the grade in effect at the end.
    HoldLastGrade,
    /// 0 % past the end.
    Flat,
}

impl BeyondEndBehavior {
    /// Every value, in declaration order.
    pub const ALL: [Self; 2] = [Self::HoldLastGrade, Self::Flat];

    /// The snapshot name.
    pub fn name(self) -> &'static str {
        match self {
            Self::HoldLastGrade => "holdLastGrade",
            Self::Flat => "flat",
        }
    }

    /// The value named `name`, if this build has it.
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.name() == name)
    }
}

/// One `(distance, grade)` point. Distance finite and non-negative; grade finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainSample {
    distance_m: f64,
    grade_fraction: f64,
}

impl TerrainSample {
    /// A validated sample.
    pub fn new(distance_m: f64, grade_fraction: f64) -> Result<Self> {
        check_finite_non_negative(distance_m, "distanceM")?;
        check_finite(grade_fraction, "gradeFraction")?;
        Ok(Self {
            distance_m,
            grade_fraction,
        })
    }

    /// Distance along the course, in metres.
    pub fn distance_m(&self) -> f64 {
        self.distance_m
    }

    /// Grade as rise/run.
    pub fn grade_fraction(&self) -> f64 {
        self.grade_fraction
    }
}

/// A frozen, serializable course profile: the terrain half of a self-contained snapshot.
///
/// Samples are non-empty, start at 0 m, and strictly increase; `length_m`, when present, is
/// positive and not before the last sample.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationTerrain {
    samples: Vec<TerrainSample>,
    interpolation: TerrainInterpolation,
    beyond_end: BeyondEndBehavior,
    length_m: Option<f64>,
}

impl SimulationTerrain {
    /// A validated terrain.
    pub fn new(
        samples: Vec<TerrainSample>,
        interpolation: TerrainInterpolation,
        beyond_end: BeyondEndBehavior,
        length_m: Option<f64>,
    ) -> Result<Self> {
        let Some(first) = samples.first() else {
            return Err(RaceSimulationError::validation(
                C::TerrainNotOrdered,
                "terrain needs at least one sample",
                "samples",
            ));
        };
        if first.distance_m != 0.0 {
            return Err(RaceSimulationError::validation(
                C::TerrainNotOrdered,
                format!(
                    "the first terrain sample must be at 0 m, got {}",
                    format_number(first.distance_m)
                ),
                "samples[0].distanceM",
            ));
        }
        for i in 1..samples.len() {
            if samples[i].distance_m <= samples[i - 1].distance_m {
                return Err(RaceSimulationError::validation(
                    C::TerrainNotOrdered,
                    format!(
                        "terrain samples must strictly increase in distance: {} does not follow {}",
                        format_number(samples[i].distance_m),
                        format_number(samples[i - 1].distance_m)
                    ),
                    format!("samples[{i}].distanceM"),
                ));
            }
        }
        if let Some(length) = length_m {
            check_finite_positive(length, "lengthM")?;
            let last = samples[samples.len() - 1].distance_m;
            if length < last {
                return Err(RaceSimulationError::validation(
                    C::TerrainLengthBeforeLastSample,
                    format!(
                        "lengthM ({}) is before the last sample at {} m",
                        format_number(length),
                        format_number(last)
                    ),
                    "lengthM",
                ));
            }
        }
        Ok(Self {
            samples,
            interpolation,
            beyond_end,
            length_m,
        })
    }

    /// A single-sample course of constant grade, with no length.
    pub fn flat(grade_fraction: f64) -> Result<Self> {
        Self::new(
            vec![TerrainSample::new(0.0, grade_fraction)?],
            TerrainInterpolation::Step,
            BeyondEndBehavior::HoldLastGrade,
            None,
        )
    }

    /// The samples, strictly ordered.
    pub fn samples(&self) -> &[TerrainSample] {
        &self.samples
    }

    /// Between-sample behaviour.
    pub fn interpolation(&self) -> TerrainInterpolation {
        self.interpolation
    }

    /// Past-the-end behaviour.
    pub fn beyond_end(&self) -> BeyondEndBehavior {
        self.beyond_end
    }

    /// The explicit course length, if any.
    pub fn explicit_length_m(&self) -> Option<f64> {
        self.length_m
    }

    /// Where the course ends: the explicit length, otherwise the last sample.
    pub fn end_distance_m(&self) -> f64 {
        self.length_m
            .unwrap_or(self.samples[self.samples.len() - 1].distance_m)
    }

    /// The grade at `distance_m`. Total: below 0 reads as 0, NaN as the start, and past the end
    /// follows [`BeyondEndBehavior`].
    pub fn grade_at(&self, distance_m: f64) -> f64 {
        if distance_m.is_nan() || distance_m <= 0.0 {
            return self.samples[0].grade_fraction;
        }
        let end = self.end_distance_m();
        if distance_m > end {
            return match self.beyond_end {
                BeyondEndBehavior::HoldLastGrade => self.grade_within_course(end),
                BeyondEndBehavior::Flat => 0.0,
            };
        }
        self.grade_within_course(distance_m)
    }

    fn grade_within_course(&self, distance_m: f64) -> f64 {
        let i = self.index_at_or_before(distance_m);
        let sample = self.samples[i];
        if self.interpolation == TerrainInterpolation::Step || i == self.samples.len() - 1 {
            return sample.grade_fraction;
        }
        let next = self.samples[i + 1];
        let span = next.distance_m - sample.distance_m;
        let t = (distance_m - sample.distance_m) / span;
        sample.grade_fraction + t * (next.grade_fraction - sample.grade_fraction)
    }

    fn index_at_or_before(&self, distance_m: f64) -> usize {
        let mut low = 0;
        let mut high = self.samples.len() - 1;
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            if self.samples[mid].distance_m <= distance_m {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        low
    }
}

impl TerrainSource for SimulationTerrain {
    fn grade_at_distance(&self, distance_m: f64) -> f64 {
        self.grade_at(distance_m)
    }

    fn length_m(&self) -> Option<f64> {
        self.length_m
    }
}
