#!/usr/bin/env python3
"""Initialize a repository created from the AI development template."""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def ask(label: str, default: str = "") -> str:
    suffix = f" [{default}]" if default else ""
    value = input(f"{label}{suffix}: ").strip()
    return value or default


def replace(path: pathlib.Path, substitutions: dict[str, str]) -> None:
    text = path.read_text(encoding="utf-8")
    for old, new in substitutions.items():
        text = text.replace(old, new)
    path.write_text(text, encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", help="Project name")
    parser.add_argument("--description", help="One-line project description")
    parser.add_argument("--owner", help="Project owner or copyright holder")
    parser.add_argument("--default-branch", help="Default branch name")
    parser.add_argument(
        "--non-interactive",
        action="store_true",
        help="Fail rather than prompt for missing required values",
    )
    return parser.parse_args()


def resolve(value: str | None, label: str, default: str, non_interactive: bool) -> str:
    if value is not None:
        return value.strip()
    if non_interactive:
        if default:
            return default
        raise SystemExit(f"Missing required option: {label}")
    return ask(label, default)


def main() -> None:
    args = parse_args()
    project_name = resolve(args.name, "Project name", ROOT.name, args.non_interactive)
    description = resolve(args.description, "One-line description", "", args.non_interactive)
    owner = resolve(args.owner, "Project owner", project_name, args.non_interactive)
    default_branch = resolve(args.default_branch, "Default branch", "main", args.non_interactive)

    substitutions = {
        "{{PROJECT_NAME}}": project_name,
        "{{PROJECT_DESCRIPTION}}": description,
        "{{PROJECT_OWNER}}": owner,
    }
    replace(ROOT / ".ai" / "project.yml", substitutions)

    project_file = ROOT / ".ai" / "project.yml"
    text = project_file.read_text(encoding="utf-8")
    text = re.sub(r'default_branch: "main"', f'default_branch: "{default_branch}"', text)
    text = text.replace('status: "bootstrap"', 'status: "active"')
    project_file.write_text(text, encoding="utf-8")

    workflow_file = ROOT / ".github" / "workflows" / "quality.yml"
    workflow = workflow_file.read_text(encoding="utf-8")
    workflow = workflow.replace("branches: [main]", f"branches: [{default_branch}]")
    workflow_file.write_text(workflow, encoding="utf-8")

    license_file = ROOT / "LICENSE"
    if license_file.exists():
        replace(
            license_file,
            {
                "{{YEAR}}": str(dt.date.today().year),
                "{{PROJECT_OWNER}}": owner,
            },
        )

    print("Initialized project metadata.")
    print("Next: review .ai/project.yml, replace README.md, and configure repository variables and branch protection.")


if __name__ == "__main__":
    main()
