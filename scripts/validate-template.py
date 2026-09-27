#!/usr/bin/env python3
"""Static validation for template-owned configuration files."""

from __future__ import annotations

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    required = [
        "AGENTS.md",
        "CLAUDE.md",
        ".ai/project.yml",
        ".claude/settings.json",
        ".github/workflows/quality.yml",
        ".github/workflows/local-proxmox-ci.yml",
        ".github/codex/prompts/review.md",
        "scripts/ci.sh",
        "scripts/setup-local-ci-labels.sh",
    ]
    for relative in required:
        if not (ROOT / relative).is_file():
            fail(f"missing required file: {relative}")

    with (ROOT / ".claude/settings.json").open(encoding="utf-8") as handle:
        json.load(handle)

    for skill in (ROOT / ".claude/skills").glob("*/SKILL.md"):
        text = skill.read_text(encoding="utf-8")
        if not text.startswith("---\n") or "\ndescription:" not in text:
            fail(f"invalid skill frontmatter: {skill.relative_to(ROOT)}")

    placeholders = []
    allowed_placeholder_files = {
        ".ai/project.yml",
        "LICENSE",
        "scripts/bootstrap.py",
        "scripts/validate-template.py",
        "scripts/ci.sh",
    }
    for path in ROOT.rglob("*"):
        if not path.is_file() or ".git" in path.parts:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        relative = str(path.relative_to(ROOT))
        if "{{PROJECT_" in text and relative not in allowed_placeholder_files:
            placeholders.append(relative)

    if placeholders:
        fail("unexpected project placeholders in: " + ", ".join(placeholders))

    print("Template validation passed")


if __name__ == "__main__":
    main()
