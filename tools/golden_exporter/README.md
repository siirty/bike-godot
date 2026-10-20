# Golden-vector exporter

`export_golden_vectors.dart` records outputs of the Dart prototype
([bike-training-app](https://github.com/siirty/bike-training-app)) into `fixtures/golden/*.json`.
The Rust crates' `tests/golden.rs` suites must reproduce them — bit-exactly for doubles, and
byte-for-byte for race snapshots.

The script is kept here for provenance; it runs inside a prototype checkout, where its package
dependencies resolve:

```sh
cp tools/golden_exporter/export_golden_vectors.dart <bike-training-app>/packages/race_engine/tool/
cd <bike-training-app>/packages/race_engine
dart pub get && dart run build_runner build --delete-conflicting-outputs   # or: melos bootstrap
dart --enable-asserts run tool/export_golden_vectors.dart <bike-godot>/fixtures/golden
```

`--enable-asserts` matters: the prototype enforces some model invariants only with `assert`, and
its own test suite runs with asserts on. Vectors were recorded from prototype commit
`25f6213` (branch `godot-preview`).

Regenerating vectors is a deliberate act: a changed vector means the reference changed, and the
Rust port must change with it (or the difference must be recorded in
`docs/port/divergences.md`).
