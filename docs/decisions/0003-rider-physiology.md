# ADR 0003: finite rider physiology and overlap-aware drafting

- Status: accepted
- Date: 2026-10-20

Software riders use Skiba differential W′ recovery, fixed CP/W′/Pmax capability and a
reserve-dependent sprint ceiling. Capability is immutable configuration; reserve is continuation
state. Motion uses delivered power after capability limiting. The sprint ceiling and fixture
values are explicit simulation approximations pending calibration.

Drafting requires longitudinal separation beyond a configurable minimum gap in every eligible
slot. The default minimum is 2.5 m center-to-center. A zero minimum remains available for
historical physics-reference tests.

Snapshot schema v3 records these quantities and rejects older schemas. Historical Dart bytes
remain rejection fixtures, and test-only conversions preserve their numerical oracles. Seeded
pack policies are implemented before utility tactics; the latter remain a subsequent increment.

Verification covers analytic energy accounting and recovery, timestep partitioning, actual-power
motion, reserve bounds, exact snapshot continuation, overlap boundaries and the seeded corpus.
