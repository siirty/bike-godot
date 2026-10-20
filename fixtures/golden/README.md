# Frozen regression vectors

These JSON files were recorded during migration from `siirty/bike-training-app`, source commit
`25f6213` on `godot-preview`. They are self-contained data, used by Rust tests without executing
or fetching that repository. The exporter was retired by ADR 0004. SHA-256 hashes are in
`checksums.json`; `python3 scripts/check-fixtures.py` verifies them.

Keep these historical observations unchanged. New numerical behavior is checked with independently
specified examples, analytical calculations, convergence budgets, properties and fixed-seed kernel
runs. Do not regenerate fixtures from production Rust output or bring back the old toolchain.
Legacy snapshot examples are test inputs only; production still rejects unsupported schemas.
