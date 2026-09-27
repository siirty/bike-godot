#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

log() { printf '\n==> %s\n' "$*"; }
has_command() { command -v "$1" >/dev/null 2>&1; }

ran=0

# The source template has no application stack of its own. While its project
# placeholders are still present, exercise the template-specific invariants and
# the contract grammar regression suite, then count those as this repository's
# deterministic checks. Generated projects replace the placeholders and fall
# through to their actual stack checks below.
if grep -Fq '{{PROJECT_NAME}}' .ai/project.yml; then
  log "Validating the source template"
  python3 ./scripts/validate-template.py
  ran=1
fi

if [[ -x ./scripts/project-ci.sh ]]; then
  log "Running project-specific scripts/project-ci.sh"
  exec ./scripts/project-ci.sh
fi

if [[ -f Makefile ]] && grep -Eq '^[[:space:]]*ci[[:space:]]*:' Makefile; then
  log "Running Makefile target: ci"
  exec make ci
fi

run_node_script() {
  local script="$1"
  node -e 'const p=require("./package.json"); process.exit(p.scripts && p.scripts[process.argv[1]] ? 0 : 1)' "$script" || return 0
  log "Node script: $script"
  node_checks=$((node_checks + 1))
  case "$NODE_PM" in
    pnpm) pnpm run "$script" ;;
    yarn) yarn "$script" ;;
    bun) bun run "$script" ;;
    npm) npm run "$script" ;;
  esac
}

if [[ -f package.json ]]; then
  ran=1
  node_checks=0
  log "Detected Node.js project"
  if [[ -f pnpm-lock.yaml ]]; then
    NODE_PM=pnpm
    has_command corepack && corepack enable
    pnpm install --frozen-lockfile
  elif [[ -f yarn.lock ]]; then
    NODE_PM=yarn
    has_command corepack && corepack enable
    yarn install --immutable
  elif [[ -f bun.lock || -f bun.lockb ]]; then
    NODE_PM=bun
    bun install --frozen-lockfile
  else
    NODE_PM=npm
    if [[ -f package-lock.json ]]; then npm ci; else npm install; fi
  fi
  for script in format:check lint typecheck test build; do
    run_node_script "$script"
  done
  if [[ "$node_checks" -eq 0 ]]; then
    echo "Node project has no recognized quality scripts (format:check, lint, typecheck, test, build)." >&2
    echo "Add scripts/project-ci.sh or a Makefile ci target." >&2
    exit 2
  fi
fi

if [[ -f pyproject.toml || -f requirements.txt ]]; then
  ran=1
  python_checks=0
  log "Detected Python project"
  PYRUN=()
  if [[ -f uv.lock ]] && has_command uv; then
    uv sync --frozen
    PYRUN=(uv run)
  elif [[ -f requirements.txt ]]; then
    python -m pip install -r requirements.txt
    PYRUN=(python -m)
  fi

  if [[ -f pyproject.toml ]] && grep -Eq '^\[tool\.ruff' pyproject.toml && has_command ruff; then
    ruff check .
    ruff format --check .
    python_checks=$((python_checks + 1))
  elif [[ ${#PYRUN[@]} -gt 0 ]] && "${PYRUN[@]}" ruff --version >/dev/null 2>&1; then
    "${PYRUN[@]}" ruff check .
    "${PYRUN[@]}" ruff format --check .
    python_checks=$((python_checks + 1))
  fi

  if [[ -f pyproject.toml ]] && grep -Eq '^\[tool\.(mypy|pyright)' pyproject.toml; then
    if has_command mypy; then
      mypy .
      python_checks=$((python_checks + 1))
    elif [[ ${#PYRUN[@]} -gt 0 ]]; then
      "${PYRUN[@]}" mypy .
      python_checks=$((python_checks + 1))
    fi
  fi

  if has_command pytest; then
    pytest
    python_checks=$((python_checks + 1))
  elif [[ ${#PYRUN[@]} -gt 0 ]] && "${PYRUN[@]}" pytest --version >/dev/null 2>&1; then
    "${PYRUN[@]}" pytest
    python_checks=$((python_checks + 1))
  fi

  if [[ "$python_checks" -eq 0 ]]; then
    echo "Python project has no recognized configured checks (ruff, mypy, pyright, pytest)." >&2
    echo "Add scripts/project-ci.sh or a Makefile ci target." >&2
    exit 2
  fi
fi

if [[ -f Cargo.toml ]]; then
  ran=1
  log "Detected Rust project"
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all-features
fi

if [[ -f go.mod ]]; then
  ran=1
  log "Detected Go project"
  test -z "$(gofmt -l .)" || { echo "gofmt changes required"; gofmt -l .; exit 1; }
  go vet ./...
  go test ./...
fi

if compgen -G '*.sln' >/dev/null || compgen -G '*.csproj' >/dev/null; then
  ran=1
  log "Detected .NET project"
  dotnet restore
  dotnet build --no-restore --configuration Release
  dotnet test --no-build --configuration Release
fi

if [[ "$ran" -eq 0 ]]; then
  cat >&2 <<'EOF'
No supported project stack or explicit CI command was detected.

Add one of:
- executable scripts/project-ci.sh (recommended), or
- a Makefile `ci` target, or
- a supported manifest: package.json, pyproject.toml, Cargo.toml, go.mod, *.sln, *.csproj.
EOF
  exit 2
fi

log "All detected checks completed"
