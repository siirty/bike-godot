# Public Template and Practice Evaluation

Research date: 2026-07-30

## Conclusion

No single well-adopted public repository exactly combines a project-agnostic GitHub template, Claude Code as implementer, subscription-authenticated Codex on a self-hosted runner, deterministic CI, and controlled ready-for-review triggering.

The strongest existing bases solve different layers:

| Project | Adoption signal | Strength | Gap for this template |
|---|---:|---|---|
| GitHub Spec Kit | ~124k GitHub stars at review time | Mature specification-driven workflow and broad coding-agent integrations | Toolkit rather than a minimal general repository baseline; no opinionated Claude-implementer/Codex-reviewer CI split |
| Claude Code Development Kit | ~1.4k stars | Closest direct precedent for Claude implementation plus Codex second opinion; concise context scaffolding | Claude-centric installer and workflow rather than a neutral GitHub template with hardened CI review publishing |
| Claude Code Templates | Large catalog of agents, commands, hooks, MCPs, and settings | Good discovery/distribution mechanism | A component marketplace, not a coherent minimal project baseline |
| Everything Claude Code | Broad production-oriented agent/skill/hook collection | Useful catalogue of planning, testing, security, documentation, and verification patterns | Intentionally extensive and workflow-specific; too much default context and machinery for a project-agnostic starter |
| Awesome Agentic AI Coding Template | Directly targets multi-agent project templates | Broad cross-tool file coverage | Very low adoption at review time and more configuration breadth than validated operating practice |

The resulting design adopts:

- Spec Kit's requirement-first discipline without importing its full lifecycle;
- AGENTS.md as the shared cross-agent instruction file;
- Claude Code's native project skills and subagents for procedural workflows;
- Claude Code Development Kit's independent second-model concept;
- official Codex read-only non-interactive execution and custom review rules;
- a draft-to-ready trigger plus explicit label-based re-review instead of review after every push.

## Typical reusable development skills

Public collections and official documentation converge on several recurring skill categories:

1. **Planning and scoped implementation** — translate a requirement into a plan, inspect existing patterns, implement incrementally, and constrain scope.
2. **Testing and verification** — derive risk-based tests, run deterministic checks, inspect the diff, and report evidence.
3. **Code review** — focus on correctness, regressions, security, test gaps, and operational consequences.
4. **Security review** — apply a threat model to auth, data, external input, secrets, dependencies, and dangerous operations.
5. **Documentation maintenance** — update docs with code and structure content for its audience and purpose.
6. **Systematic debugging** — reproduce, isolate, form hypotheses, test them, and prevent recurrence.
7. **Dependency management** — justify new packages, assess compatibility and supply-chain risk, and update lockfiles correctly.
8. **Architecture/ADR support** — make trade-offs explicit and preserve durable decisions.
9. **Build and CI failure resolution** — diagnose deterministic failures rather than suppressing checks.
10. **End-to-end/UI verification** — use browser or integration tooling when the product has a user interface.

A template should not install every popular skill. Skills add value when they encode a repeated procedure that is specific enough to improve outcomes. This template includes five universal procedures and two read-only specialists. Stack- or domain-specific skills should be added only after the project exists.

## Documentation skill rationale

The documentation skill uses the four Diátaxis purposes—tutorial, how-to, reference, and explanation—to prevent mixed-purpose documents. It also requires one source of truth, verified examples, audience-specific prerequisites, operational recovery steps, and ADRs for durable design rationale.

## Primary sources

- GitHub Spec Kit: https://github.com/github/spec-kit
- Spec Kit integrations: https://github.github.com/spec-kit/reference/integrations.html
- AGENTS.md format: https://agents.md/
- Claude Code skills: https://code.claude.com/docs/en/skills
- Claude Code subagents: https://code.claude.com/docs/en/sub-agents
- Claude Code best practices: https://code.claude.com/docs/en/best-practices
- Anthropic skills examples: https://github.com/anthropics/skills
- Claude Code Development Kit: https://github.com/peterkrueck/Claude-Code-Development-Kit
- Claude Code Templates: https://github.com/davila7/claude-code-templates
- Codex AGENTS.md: https://learn.chatgpt.com/docs/agent-configuration/agents-md
- Codex non-interactive mode: https://learn.chatgpt.com/docs/non-interactive-mode
- Codex CI/CD account authentication: https://learn.chatgpt.com/docs/auth/ci-cd-auth
- Codex GitHub Action: https://learn.chatgpt.com/docs/github-action
- Diátaxis: https://diataxis.fr/
