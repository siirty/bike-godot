---
name: verify-change
description: Verify a code change with risk-based tests, static checks, and explicit evidence before review.
argument-hint: "[change or risk area]"
disable-model-invocation: true
---

Verify `$ARGUMENTS` without changing product behavior unless a defect is found and the task permits a fix.

1. Identify changed behavior, interfaces, data, permissions, concurrency, and failure modes.
2. Map each material risk to an existing or new verification method.
3. Run the narrowest relevant checks first, then the project CI command.
4. Confirm tests would detect the intended regression; do not rely only on green output.
5. Inspect logs and warnings. Treat flaky, skipped, quarantined, and unexpectedly fast tests as evidence gaps.
6. Check the final diff for generated artifacts, debug output, secrets, and unintended changes.
7. Report every command, its result, and anything not executed.

A verification report must distinguish:

- passed checks;
- failed checks;
- checks not applicable;
- checks not run and why;
- residual risks requiring human or environment-specific validation.
