# AI Review Policy

Codex review is an additional advisory signal. It does not replace tests, static analysis, security tooling, domain review, or human responsibility.

## Normal cadence

- First review: automatically when a draft pull request becomes ready for review.
- Re-review: manually add the `ai-review` label after addressing findings and rerunning CI.
- Avoid review on every push; review coherent revisions.

## Finding handling

Each finding must be classified by the implementer or human as:

- **accepted** — defect is valid and will be fixed;
- **rejected** — finding is invalid, with concrete rationale;
- **deferred** — valid but outside scope, with a follow-up issue;
- **needs decision** — requires product, architecture, security, or domain judgment.

## Merge policy

AI review is advisory by default. Make it a required check only after measuring its false-positive rate, missed-defect rate, latency, and operational reliability on the actual project.
