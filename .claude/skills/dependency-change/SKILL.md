---
name: dependency-change
description: Evaluate and implement dependency additions or upgrades with necessity, compatibility, licensing, and supply-chain checks.
argument-hint: "[dependency and intended use]"
disable-model-invocation: true
---

Evaluate `$ARGUMENTS` before changing manifests or lockfiles.

1. State the capability required and whether the standard library or an existing dependency already provides it.
2. Confirm the dependency is directly used and proportionate to the need.
3. Check compatibility with the project's runtime, frameworks, licenses, and deployment targets.
4. Review maintenance signals and known security advisories using authoritative sources when internet access is available.
5. Prefer a narrow, supported version range consistent with the ecosystem's lockfile strategy.
6. Make the manifest and lockfile change together.
7. Run affected tests and build steps.
8. Document new configuration, operational behavior, transitive risk, or migration requirements.

Never add a package only to avoid implementing a small, stable function already covered by the project.
