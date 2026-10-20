---
name: workout-file-formats
description: Reference for the workout and route file formats this app imports — .zwo (Zwift), .erg/.mrc (TrainerRoad/CompuTrainer), .fit (Garmin, workouts and courses), .tcx, and .gpx. Use whenever implementing or debugging crates/workout_import, or anything that needs to know what these files look like and what they can express.
---

# Workout & route file format reference

`crates/workout_import` converts all of these into the shared `WorkoutPlan` (structured
intervals: target power/duration, usually as %FTP) and/or `RouteProfile` (geography: lat/lon/
elevation over distance) models. Verify exact schemas against sample files and, where one exists,
the authoritative spec/SDK before finalizing a parser — formats below are summarized for
orientation, not byte-exact.

## Structured workout formats (→ `WorkoutPlan`)

- **`.zwo` (Zwift workout file)** — XML. Root `<workout_file>` with metadata
  (`<author>`, `<name>`, `<description>`, `<sportType>`) and a `<workout>` element containing a
  sequence of interval elements: `<Warmup Duration="600" PowerLow="0.5" PowerHigh="0.75"/>`,
  `<SteadyState Duration="600" Power="0.75"/>`, `<IntervalsT Repeat="4" OnDuration="300"
  OffDuration="300" OnPower="1.0" OffPower="0.5"/>`, `<Ramp .../>`, `<FreeRide Duration=".."/>`,
  `<Cooldown .../>`. Power values are **fractions of FTP** (`1.0` = 100% FTP). Optional `Cadence`
  attributes. Simplest format here — plain XML, no binary encoding — good first parser to build
  (see `docs/architecture/build-order.md`, Slice 1).
- **`.erg` / `.mrc` (TrainerRoad / CompuTrainer legacy text formats)** — plain text, not XML.
  `[COURSE HEADER]` section (version, units, description) followed by `[COURSE DATA]`: a list of
  `minute value` pairs, linearly interpolated between points. `.erg` values are **absolute watts**;
  `.mrc` values are **%FTP/relative**. Less rigorously standardized than the others — test against
  real sample files from each source tool rather than trusting a single reference doc.
- **`.fit` workout messages** — see FIT below; same binary container as `.fit` courses, different
  global message types.

## Route/GPS formats (→ `RouteProfile`)

- **`.gpx` (GPS Exchange Format)** — XML, the simplest route format. `<gpx><trk><trkseg><trkpt
  lat=".." lon="..">` with child `<ele>` (elevation) and `<time>`. No native power/HR fields in the
  base spec — those require vendor extensions (e.g. Garmin's `TrackPointExtension` namespace).
  Route parsing is deferred to Slice 10, after the headless simulation and live integration are validated.
- **`.tcx` (Training Center XML, Garmin)** — XML. `<TrainingCenterDatabase>` with either
  `<Courses><Course><Track><Trackpoint>` or `<Activities>`; each `<Trackpoint>` has `<Time>`,
  `<Position><LatitudeDegrees>/<LongitudeDegrees></Position>`, `<AltitudeMeters>`,
  `<DistanceMeters>`, optionally `<HeartRateBpm>` and `<Cadence>`.
- **`.fit` course messages** — see FIT below.

## `.fit` (Garmin FIT SDK format) — highest-risk parser, flag for an early spike

Binary, TLV-record-based, used for **both** structured workouts and GPS courses depending on which
global messages are present:

- `workout` (global message 26) / `workout_step` (27) — structured intervals: target type
  (power/heart_rate/cadence/speed), target value or range, duration.
- `course` (31) + `lap` + `record` (20) — GPS courses: `record` messages carry
  `position_lat`/`position_long`/`altitude`/`distance`, typically sampled at 1 Hz.
- File structure: a 12-byte header (with protocol/profile version and data size), a CRC-checked
  binary body of interleaved **message definitions** and **data records** (semantic-free until
  decoded against the definitions), and a trailing CRC.

This format needs either the official Garmin FIT SDK or a compatible third-party decoder — hand-
rolling a binary parser from scratch is realistically not worth it. Per
`docs/architecture/risks.md`: confirm a usable Rust FIT-parsing crate exists before committing to
a timeline for this parser; if none does, budget for porting decode logic rather than assuming a
crate will appear.
