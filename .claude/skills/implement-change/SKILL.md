---
name: implement-change
description: Implement a scoped feature or bug fix from a task, issue, or specification using the repository workflow.
argument-hint: "[task, issue, or requirement]"
disable-model-invocation: true
---

Implement `$ARGUMENTS` as a reviewable pull-request-sized change.

1. Read `AGENTS.md`, `.ai/project.yml`, the task, relevant requirements, ADRs, code, and tests.
2. Convert the task into explicit acceptance criteria. Separate stated requirements from assumptions.
3. Explore affected code paths and existing conventions before proposing changes.
4. Write a concise plan covering implementation, tests, documentation, compatibility, and risk.
5. Implement the smallest coherent solution. Avoid unrelated cleanup.
6. Add or update tests that fail without the change and exercise meaningful error paths.
7. Update documentation and ADRs when the user-visible contract, setup, operations, or architecture changes.
8. Run the configured checks. Fix causes rather than suppressing diagnostics.
9. Inspect `git diff` and `git status` for accidental changes.
10. Report: behavior changed, files affected, commands executed, results, assumptions, and remaining risk.

Do not mark the work complete when required evidence is unavailable. State what remains unverified.
