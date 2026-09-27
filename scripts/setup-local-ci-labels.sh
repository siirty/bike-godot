#!/usr/bin/env bash
set -euo pipefail

if ! command -v gh >/dev/null 2>&1; then
  echo "GitHub CLI (gh) is required." >&2
  exit 1
fi

repo="${1:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}"

gh label create 'ci:manual-trigger' \
  --repo "$repo" \
  --description 'Run disposable local Proxmox CI once for this PR' \
  --color '1D76DB' \
  --force

gh label create 'ci:merge-check' \
  --repo "$repo" \
  --description 'Run local CI as an explicit pre-merge check' \
  --color 'FBCA04' \
  --force

echo "Local CI labels configured for $repo"
