# AI-Supported Development Workflow

## Roles

| Role | Default tool | Responsibility |
|---|---|---|
| Product/technical owner | Human | Intent, scope, acceptance criteria, risk decisions, merge decision |
| Implementer | Claude Code or another coding agent | Explore, plan, implement, test, document, prepare coherent PR |
| Deterministic verifier | CI | Build, lint, type-check, test, and stack-specific checks |
| Independent reviewer | Codex | Read-only defect review after deterministic CI succeeds |

The role separation matters more than the specific models. An independent reviewer should not inherit the implementer's private reasoning or assumptions.

## Repository initialization

Run `python scripts/bootstrap.py`, then complete `.ai/project.yml`.

Define a project-specific `scripts/project-ci.sh` or Makefile `ci` target once the stack is known. The generic dispatcher in `scripts/ci.sh` is a useful bootstrap fallback, not a substitute for deliberate project CI.

Create the normal repository labels required by the project. For the optional local Proxmox runner, run:

```bash
bash scripts/setup-local-ci-labels.sh
```

This creates:

- `ci:manual-trigger`
- `ci:merge-check`

Recommended repository variables:

| Variable | Example | Purpose |
|---|---|---|
| `CI_RUNNER_JSON` | `"ubuntu-latest"` | Runner selection for ordinary deterministic CI |
| `CODEX_RUNNER_JSON` | `["self-hosted","linux","codex-review"]` | Optional dedicated reviewer runner |
| `CODEX_REVIEW_ENABLED` | `true` | Set to `false` to disable AI review |

## Implementer workflow

Read `AGENTS.md`, `.ai/project.yml`, the task, relevant requirements, ADRs, code, and tests before editing.

A normal task should:

1. define the intended outcome and acceptance criteria;
2. inspect existing behavior and interfaces;
3. plan the smallest coherent change;
4. implement with tests and documentation;
5. run the project verification command;
6. inspect the final diff;
7. report verification evidence and residual risk.

The committed Claude skills under `.claude/skills/` encode reusable procedures for implementation, verification, documentation, security, and dependency changes. Other coding agents should follow the same repository-level constraints even if they do not consume those skill files directly.

## Pull-request lifecycle

1. Create a draft PR early for non-trivial work.
2. Push incremental work; deterministic CI runs normally.
3. When behavior, tests, documentation, and PR description are coherent, mark the PR ready for review.
4. After deterministic CI succeeds, Codex can perform a read-only independent review.
5. Validate each finding rather than applying it mechanically.
6. After material fixes, rerun CI and use the `ai-review` label for a later coherent review.
7. Merge only after a human accepts the residual risk.

## Disposable local Proxmox CI

The template integrates with the standalone `siirty/gha-proxmox-runner` project through `.github/workflows/local-proxmox-ci.yml`.

The repository workflow is intentionally thin. The runner controller owns the security policy and independently validates the queued job before provisioning a VM.

Current policy:

| Trigger | Base restriction | Behavior |
|---|---|---|
| PR merged | Must target `main` | Run local CI automatically after the merge |
| `ci:manual-trigger` added | None | Run local CI once for the current PR head |
| `ci:merge-check` added | None | Run local CI for the current PR head; informational for now |

Adding either label does not grant a repository persistent access to a runner. Each authorized job receives a unique JIT runner and the VM is destroyed after the job.

A repository must also be included in the runner controller's GitHub App installation before it can use the service.

## Codex reviewer runner

The optional Codex review workflow uses a dedicated self-hosted runner by default. Keep that runner separate from arbitrary build workloads and keep Codex read-only.

For public repositories, shared infrastructure, or stronger credential isolation, use an API-backed review action instead of subscription-authenticated persistent runner credentials.

## Branch protection

Initially require deterministic CI. Keep Codex advisory until its reliability and value have been measured on the project.

The local `ci:merge-check` label is also informational initially. If it later becomes a real merge gate, require a status check bound to the current PR head SHA so newer commits cannot inherit an obsolete successful check.
