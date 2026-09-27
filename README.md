# AI-Supported Development Template

A project-agnostic GitHub template for disciplined AI-assisted software development.

The default operating model is:

1. A human defines intent and acceptance criteria.
2. A coding agent explores, plans, implements, tests, and documents the change.
3. Deterministic CI runs project checks.
4. Codex can perform an independent, read-only pull-request review.
5. A human evaluates the evidence and decides whether to merge.

This repository deliberately keeps the core small. Add stack- and domain-specific rules only after the project exists.

## Create a project

Use **Use this template** on GitHub, then run:

```bash
python scripts/bootstrap.py
```

Then:

1. Review `.ai/project.yml`.
2. Replace this README with the project README.
3. Configure branch protection for the default branch.
4. Run `bash scripts/setup-local-ci-labels.sh` if the repository should use the disposable Proxmox CI runner.
5. Configure the optional Codex reviewer runner or disable it with `CODEX_REVIEW_ENABLED=false`.
6. Open non-trivial work as a draft pull request and mark it ready when the implementation is coherent.

Detailed setup: [`docs/development/AI_WORKFLOW.md`](docs/development/AI_WORKFLOW.md)

## Core files

| File | Purpose |
|---|---|
| `AGENTS.md` | Shared, tool-agnostic instructions and review rules |
| `CLAUDE.md` | Claude Code implementer role and workflow |
| `.claude/skills/` | Reusable implementation, verification, documentation, security, and dependency procedures |
| `.ai/project.yml` | Compact project facts and project-specific commands |
| `.ai/task-template.md` | Generic structure for scoped implementation tasks |
| `.ai/review-policy.md` | Advisory independent-review policy |
| `.github/workflows/quality.yml` | Deterministic project checks plus optional Codex review |
| `.github/workflows/local-proxmox-ci.yml` | Thin trigger layer for the standalone `gha-proxmox-runner` service |
| `.github/codex/prompts/review.md` | Independent reviewer contract loaded from the base branch |
| `scripts/ci.sh` | Project-agnostic CI dispatcher with stack detection and an explicit override path |
| `scripts/setup-local-ci-labels.sh` | Creates the two optional local-CI trigger labels |

## CI model

The template deliberately separates ordinary repository CI from the home runner.

`quality.yml` runs deterministic checks for normal pull-request feedback. Projects can keep those checks on GitHub-hosted runners or configure another runner through `CI_RUNNER_JSON`.

The optional local runner integration is provided by [`siirty/gha-proxmox-runner`](https://github.com/siirty/gha-proxmox-runner). Its controller independently authorizes only:

- a PR merged into `main` — automatic local CI;
- `ci:manual-trigger` — one explicit local CI run on any PR/base branch;
- `ci:merge-check` — an explicit pre-merge run on any PR/base branch.

`ci:merge-check` is informational for now and is intentionally distinct so it can later become a required merge gate.

## Review trigger policy

- Deterministic CI runs on every pull-request update.
- Codex review runs automatically when a draft PR becomes **Ready for review**.
- Add the `ai-review` label to request a later re-review.
- Codex is advisory by default; deterministic CI remains the merge gate unless the project explicitly changes that policy.

## Template principles

- Specifications and acceptance criteria precede implementation.
- Shared instructions stay concise; procedures live in skills.
- Verification evidence is required, not merely a claim that something works.
- The implementer and reviewer can be different models and roles.
- AI-generated changes remain attributable and reviewable through ordinary Git history.
- Secrets, production access, deployments, and destructive operations require explicit human control.
- Infrastructure services such as local CI remain separate products rather than being embedded into the project template.

## License

The template files are provided under the MIT License. Replace the copyright placeholder when bootstrapping a project.
