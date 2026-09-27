---
name: security-check
description: Perform a focused security review of changed code, configuration, dependencies, and data flows.
argument-hint: "[change or threat surface]"
disable-model-invocation: true
context: fork
agent: security-reviewer
background: false
---

Review `$ARGUMENTS` using the security-reviewer subagent.

Scope the review to plausible risks introduced or exposed by the change. Trace trust boundaries, attacker-controlled inputs, authorization decisions, secrets, sensitive data, external calls, parsing, serialization, file paths, commands, and dependency changes.

Return concrete findings with an execution path, impact, confidence, and proportionate mitigation. Separate confirmed defects from hardening suggestions. Do not modify files.
