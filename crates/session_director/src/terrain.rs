use workout_import::WorkoutSegment;

/// The course's grade as a function of distance. Implementations are pure and deterministic, and
/// accept any distance, including past [`Self::length_m`].
pub trait TerrainSource {
    /// Grade as rise/run at `distance_m`.
    fn grade_at_distance(&self, distance_m: f64) -> f64;
    /// The course length in metres, or `None` for an endless course. Advisory only.
    fn length_m(&self) -> Option<f64>;
}

/// One constant grade, all the way along.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlatTerrain {
    /// The constant grade.
    pub grade_fraction: f64,
}

impl TerrainSource for FlatTerrain {
    fn grade_at_distance(&self, _distance_m: f64) -> f64 {
        self.grade_fraction
    }
    fn length_m(&self) -> Option<f64> {
        None
    }
}

/// One `(distance, grade)` point of a synthesized course.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradePoint {
    /// Distance along the course, in metres. Non-negative.
    pub distance_m: f64,
    /// Grade as rise/run.
    pub grade_fraction: f64,
}

/// One workout segment's span of the course: start inclusive, end exclusive.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentDistanceRange {
    /// Index of the segment in the source plan.
    pub segment_index: usize,
    /// The segment itself.
    pub segment: WorkoutSegment,
    /// Where it starts, in metres.
    pub start_distance_m: f64,
    /// Where the next one starts, in metres. The last range's end is the course length.
    pub end_distance_m: f64,
}

/// A frozen route plus the workout↔distance mapping, so a later race event can say "at D, inside
/// segment S, on grade G" without re-walking the plan. A step profile: grade holds from each
/// sample to the next.
#[derive(Debug, Clone, PartialEq)]
pub struct SynthesizedTerrain {
    samples: Vec<GradePoint>,
    segment_ranges: Vec<SegmentDistanceRange>,
}

impl SynthesizedTerrain {
    /// Validates and builds a terrain: samples non-empty, starting at 0 m and strictly increasing;
    /// ranges ordered, non-overlapping, each of positive width, and ending at the last sample.
    pub fn new(
        samples: Vec<GradePoint>,
        segment_ranges: Vec<SegmentDistanceRange>,
    ) -> Result<Self, String> {
        let Some(first) = samples.first() else {
            return Err("terrain needs at least one sample".into());
        };
        if first.distance_m != 0.0 {
            return Err("the first sample must be at 0 m".into());
        }
        let mut prev = -1.0;
        for s in &samples {
            if s.distance_m <= prev {
                return Err(format!(
                    "terrain samples must strictly increase in distance: got {} after {prev}",
                    s.distance_m
                ));
            }
            prev = s.distance_m;
        }
        let mut prev_end = -1.0;
        for r in &segment_ranges {
            if r.start_distance_m < prev_end {
                return Err(format!(
                    "segment ranges must be ordered and non-overlapping: {} after {prev_end}",
                    r.start_distance_m
                ));
            }
            if !(r.start_distance_m >= 0.0 && r.end_distance_m > r.start_distance_m) {
                return Err("a segment range must start at or after 0 m and have width".into());
            }
            prev_end = r.end_distance_m;
        }
        let course_end = samples.last().expect("non-empty").distance_m;
        if let Some(last) = segment_ranges.last() {
            if last.end_distance_m != course_end {
                return Err(format!(
                    "final segment end ({} m) must equal the course length ({course_end} m)",
                    last.end_distance_m
                ));
            }
        }
        Ok(Self {
            samples,
            segment_ranges,
        })
    }

    /// The course points, strictly ordered, starting at 0 m.
    pub fn samples(&self) -> &[GradePoint] {
        &self.samples
    }

    /// Per-segment course spans, in plan order.
    pub fn segment_ranges(&self) -> &[SegmentDistanceRange] {
        &self.segment_ranges
    }

    /// The range containing `distance_m`, if it is on the course.
    pub fn segment_range_at(&self, distance_m: f64) -> Option<&SegmentDistanceRange> {
        self.segment_ranges
            .iter()
            .find(|r| distance_m >= r.start_distance_m && distance_m < r.end_distance_m)
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

impl TerrainSource for SynthesizedTerrain {
    fn grade_at_distance(&self, distance_m: f64) -> f64 {
        let first = self.samples[0].grade_fraction;
        let last = self.samples[self.samples.len() - 1];
        if distance_m.is_nan() || distance_m <= 0.0 {
            return first;
        }
        let end = self
            .segment_ranges
            .last()
            .map_or(last.distance_m, |r| r.end_distance_m);
        if distance_m >= end {
            return last.grade_fraction;
        }
        self.samples[self.index_at_or_before(distance_m)].grade_fraction
    }

    fn length_m(&self) -> Option<f64> {
        self.segment_ranges.last().map(|r| r.end_distance_m)
    }
}
