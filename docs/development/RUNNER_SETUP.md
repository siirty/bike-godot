# Self-hosted runner setup (gh-runner)

The Rust workspace consumes the private `siirty/bike-training-core` as a git dependency.
On the self-hosted runner this is authenticated **runner-side** (not in the workflow),
because OAuth tokens cannot edit workflow files without the `workflow` scope.

One-time, as the user the runner service runs as:

```bash
# 1. Let cargo clone private deps through the git CLI
mkdir -p ~/.cargo
cat > ~/.cargo/config.toml <<'TOML'
[net]
git-fetch-with-cli = true
TOML

# 2. Give git a credential entry for github.com (x-access-token works for PATs)
#    Use a fine-grained PAT with read access to siirty/bike-training-core.
git config --global credential.helper store
# then create ~/.git-credentials containing one line:
# https://x-access-token:<TOKEN>@github.com

# 3. Ensure godot + rustc/cargo are installed and on PATH for that user
#    (this container's reference setup: godot 4.3 at /usr/local/bin/godot,
#     rustup stable, xvfb + libgl1-mesa-dri for headless render checks).
```

The runner (`gh-runner`, labels self-hosted/linux/X64) is registered to the repo at
Settings → Actions → Runners. CI green is verified by the queued workflow_dispatch run.
