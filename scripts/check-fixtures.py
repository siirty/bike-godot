#!/usr/bin/env python3
"""Verify that historical regression evidence has not been silently rewritten."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1] / "fixtures" / "golden"
expected = json.loads((root / "checksums.json").read_text())
actual = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in root.glob("*.json")
          if p.name != "checksums.json"}
if actual != expected:
    raise SystemExit("Frozen regression fixtures changed; review the contract and provenance.")
print(f"Verified {len(actual)} frozen regression fixtures")
