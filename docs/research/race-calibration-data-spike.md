# Race calibration data-access feasibility spike

## Handoff status

This document is a self-contained brief for an external developer or research agent. No prior
knowledge of the repository is required. Read this document before inspecting implementation code.

This is a **time-boxed research spike**, not a production data-ingestion feature and not a new
numbered implementation slice. It de-risks the later calibration work that follows the first
headless autonomous race.

## Objective

Prove whether one real or virtual cycling race can be converted into synchronized, reproducible
observations suitable for evaluating a headless race simulator.

The spike must answer:

1. Can rider observations be obtained at useful temporal resolution for a meaningful portion of a
   race?
2. Can they be aligned to a common clock and route coordinate?
3. Can group structure, gaps and important state transitions be derived with defensible rules?
4. Which fields are measured, reported, derived or inferred?
5. Can the extraction be repeated legally and technically?
6. Is the source useful for race behavior, physical effort, or both?
7. What should the simulator log now so its output can later be compared with this data?

A negative result is valid if it is supported by evidence and identifies the next-best source.

## Product context

The product turns structured workouts into connected tactical learning situations; it does not
seek full historical race reproduction. [TT-01–TT-10](../requirements/tactical-training.md) define
recognition/timing, player knowledge, effort, continuity and plausible progression/results.
The implemented deterministic kernel and initial tactics provide the execution baseline.

Extract representative situations and the preceding context needed to interpret them. Data may
calibrate physical or group behavior without validating perceptual learning. Record course, field,
affiliations and selections when available, and inspect the path to the result rather than merely
matching finishing order.

The eventual calibration target is **believable race development**, not reproduction of an exact
historical trajectory. Expected comparison dimensions include:

- group counts, sizes, splits and merges;
- inter-group gaps and their rates of change;
- attacks, responses, bridges, catches and riders being dropped;
- behavior conditioned on terrain, group composition and race context;
- effort and fatigue only where sufficiently reliable data exists.

Do not attempt to build the simulator, optimizer, Scene Planner or Workout Director in this spike.

## Source candidates

Evaluate access to both candidates briefly, but extract a technical sample from only one.

### Preferred pilot: consented or public Zwift race

Existing replay tooling demonstrates that authenticated Zwift activity data can contain
second-by-second field position, power, heart rate, speed, distance, rider weight and route
geometry. Private activities may be absent.

Relevant starting points:

- <https://zwifttools.azurewebsites.net/race-replay>
- <https://github.com/veelenga/zwiftpower-race-replay>

Prefer a race for which participants have consented to research/export. Do not assume that public
visibility grants permission to redistribute identifiable athlete data.

### Secondary pilot: Tour de France tracking

Tour tracking systems collect rider location and speed for the field. Public surfaces primarily
expose processed live positions, rankings, speeds and gaps; an openly downloadable historical raw
feed has not yet been verified.

Relevant starting points:

- <https://www.letour.fr/en/data>
- <https://www.tomtom.com/newsroom/behind-the-map/tour-de-france-full-of-location-tech/>
- <https://dl.acm.org/doi/10.1007/978-3-030-17274-9_10>

Tour data is potentially strong for authentic group transitions but weak for measured individual
power. Treat reconstructed power as uncertain supporting data, never as ground truth.

## Time box and stopping rule

Time box the spike to **one or two engineering days**.

Do not build a scraper farm, historical archive, production service or generalized import
framework. Stop once one source has produced a reproducible sample and the feasibility questions
have evidence-backed answers.

If access, licensing, privacy, authentication or source instability makes repeatable use
unacceptable, stop extraction and report the blocker. Do not bypass access controls.

## Minimum sample target

Aim for:

- at least 10 riders;
- at least 10 consecutive minutes;
- the source's native timestamp resolution, documented explicitly;
- one shared route or race;
- enough movement to observe at least one group transition.

A smaller sample may still prove feasibility, but the report must explain why it is sufficient and
what remains unproven.

Do not commit raw identifiable athlete data. Keep raw material outside the repository. A committed
sample must be consented and anonymized, or be a synthetic fixture derived from documented aggregate
properties.

## Data provenance classes

Every field must carry one of these provenance classes:

- **measured** — directly recorded by a rider device or race tracker;
- **reported** — supplied by an official result/profile/source but not measured in this feed;
- **derived** — deterministically calculated from measured/reported fields;
- **inferred** — estimated through a model or uncertain assumptions;
- **unknown** — unavailable or provenance cannot be established.

Never silently promote inferred power, intent or drafting state to measured data.

## Canonical observation proposal

The spike should map the source into a small source-neutral representation. The exact serialization
format is not an architecture decision; JSON Lines, CSV plus metadata, or Parquet are acceptable for
the experiment.

### Race metadata

```text
race_id
source
source_reference
route_id
start_time
sampling_description
coordinate_system
license_or_terms_status
anonymization_method
```

### Rider metadata

```text
rider_id_anonymous
team_id_anonymous?
mass_kg?
height_m?
capability_or_rank_proxies?
metadata_provenance
```

### Time-series observation

```text
race_id
timestamp_or_elapsed_ms
rider_id_anonymous
route_distance_m?
latitude?
longitude?
elevation_m?
gradient?
speed_mps?
acceleration_mps2?
race_rank?
group_id?
gap_to_group_ahead_s?
power_w?
heart_rate_bpm?
cadence_rpm?
field_provenance
missingness_flags
confidence
```

### Derived event

```text
event_id
elapsed_ms
event_type
actor_rider_ids
source_group_ids
result_group_ids
evidence
confidence
```

Initial event types may include `group_split`, `group_merge`, `attack_candidate`,
`follow_candidate`, `bridge_complete`, `catch` and `rider_dropped`. These are observations
or evidence-backed labels, not claims about unobservable rider intent.

## Required work

### 1. Access and rights assessment

For each candidate source, record:

- exact access path and whether authentication is required;
- whether historical data is available;
- whether access depends on undocumented endpoints or page scraping;
- fields and temporal resolution actually observed;
- private/missing participant behavior;
- relevant redistribution, privacy and retention restrictions;
- likelihood that the access method remains repeatable.

Distinguish verified behavior from assumptions. Link primary documentation where possible.

### 2. Select one source

Choose the source with the best combination of completeness, repeatability and lawful use. Explain
the decision. Prefer a complete, consented smaller race over a larger but incomplete or
questionably reusable field.

### 3. Extract and preserve provenance

Retain the raw source outside the repository. Produce a transformation log that records:

- retrieval time;
- source identifier;
- transformations in order;
- units and coordinate conversions;
- dropped or interpolated samples;
- anonymization;
- source and tool versions where known.

### 4. Synchronize and normalize

Map riders to:

- one elapsed-time axis;
- one route-distance coordinate where possible;
- SI units;
- explicit missing values rather than fabricated samples.

Document clock offsets, interpolation and GPS/map-matching behavior. Do not smooth away attacks or
short gaps without showing the impact of that choice.

### 5. Derive race structure

Implement or manually demonstrate transparent rules for:

- group membership;
- inter-group gaps;
- split and merge detection;
- at least one additional observable event such as a catch, bridge completion or rider being
  dropped.

The output must retain the evidence behind each label so thresholds can later change.

### 6. Assess effort information

If measured power exists, report coverage, sampling and missingness.

If using Tour tracking, demonstrate power reconstruction only on a short suitable segment, ideally
a steady climb or isolated rider. Return an interval or distribution and list assumptions for mass,
gradient, wind, rolling resistance, aerodynamics and drafting. Mark descents and dense flat
pelotons as low-confidence or non-identifiable.

Do not calibrate a drafting parameter against power inferred using that same drafting parameter.

### 7. Compare with simulator needs

Produce a short mapping between available dataset fields and the minimum future simulator log.
Identify:

- directly comparable state;
- state requiring a projection or detector;
- unavailable latent state;
- simulator parameters that this source could plausibly constrain;
- parameters that require an independent measured dataset.

## Deliverables

Return all of the following:

1. **Feasibility report** — concise answers to the seven objective questions, source comparison,
   selected source, findings, risks and recommendation.
2. **Reproduction instructions** — exact steps and prerequisites to repeat extraction on an
   authorized source.
3. **Schema/example** — the normalized schema plus a small anonymized, consented or synthetic
   example.
4. **Transformation/provenance log** — measured/reported/derived/inferred classification and all
   material assumptions.
5. **Coverage report** — rider count, duration, sampling, missingness and excluded riders/periods.
6. **Race-structure demonstration** — groups, gaps and at least one transition with inspectable
   evidence.
7. **Simulator logging recommendation** — minimal fields/events the headless kernel should expose.
8. **Go/no-go conclusion** — one of:
   - proceed with this source;
   - proceed only for a limited calibration purpose;
   - request partnership/consent before proceeding;
   - reject and use the named fallback.

Do not return a production ingestion implementation as the main deliverable.

## Acceptance criteria

The spike is complete when:

- another developer can repeat the authorized extraction from the instructions;
- the sample uses a common clock and route coordinate where the source permits it;
- measured, reported, derived and inferred fields are distinguishable;
- missing riders and samples are quantified rather than hidden;
- group/gap derivation and one transition can be inspected;
- no identifiable raw athlete dataset is committed without explicit permission;
- the report states what the source can and cannot calibrate;
- the go/no-go recommendation is supported by evidence.

Failure to obtain data does not fail the spike if access attempts, blockers and the next action are
documented clearly.

## Explicit non-goals

Do not:

- optimize the race AI;
- fit final rider profiles;
- claim exact power from GPS;
- tune final thresholds;
- create a comprehensive Tour archive;
- reverse-engineer or bypass protected access;
- commit credentials, session tokens or identifiable raw activity files;
- modify the simulation architecture based on one dataset;
- treat historical race outcome reproduction as the product objective.

## Expected decision after the spike

If a source is viable, compare its representative situation windows with the existing headless
kernel/tactics, keeping behavioral constants configurable. Include novice, experienced amateur
and elite assumptions where evidence permits; do not infer proficiency solely from FTP. Identify
which observations constrain local behavior, course-specific selection and outcome plausibility.
A telemetry source alone cannot validate player recognition or learning transfer. Build a larger
corpus after focused comparisons reveal which parameters and distributions need calibration.
