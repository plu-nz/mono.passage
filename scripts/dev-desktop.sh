#!/usr/bin/env bash
# Run the desktop app against the Vite dev server: frontend edits apply live
# through HMR; Go edits need this script to be restarted.
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_dir"

# go:embed needs desktop/dist to exist even though dev mode does not serve it.
[[ -f desktop/dist/index.html ]] || pnpm --filter @passage/desktop build

# Stop Vite (and the pnpm wrapper around it) when the app exits or on Ctrl+C.
trap 'trap - EXIT INT TERM; kill 0 2>/dev/null || true' EXIT INT TERM
pnpm --filter @passage/desktop dev:frontend &

FRONTEND_DEVSERVER_URL=http://127.0.0.1:1420 go run ./desktop
