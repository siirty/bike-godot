# ADR 0004: Own the Rust contracts and freeze migration evidence

- Status: accepted
- Date: 2026-10-20
- Supersedes: ADR 0001/0002 policies requiring the legacy application as an ongoing oracle

## Decision

Builds, tests, exports and future simulation work require only this repository, its declared Rust
dependencies and Godot. Remove the legacy-language fixture exporter. Keep recorded vectors as
immutable, checked-in historical regression evidence, with source provenance and SHA-256 hashes.
They do not need to be regenerated, and tests never execute the old application.

The Rust architecture and typed contracts are authoritative. New behavior uses independent
reference calculations, invariant/state-transition tests and deterministic seed corpora. A deliberate
contract change is documented here and verified directly; it does not require updating the old app.
Never generate expected results by running the implementation being tested.

Existing numeric grammars, ID ordering and canonical JSON encoding remain local compatibility
contracts. Their Rust implementation names describe the contract rather than its former language.
This changes neither snapshot schema v3 nor persisted workouts. Historical ADRs and migration notes
remain for provenance; they are not current instructions to run another repository.

## Verification

Run the workspace checks and Godot smoke test without a legacy checkout or toolchain. Verify frozen
fixture hashes with `python3 scripts/check-fixtures.py`; the project CI invokes this before tests.
