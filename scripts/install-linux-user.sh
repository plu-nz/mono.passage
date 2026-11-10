#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
install_root="${XDG_DATA_HOME:-$HOME/.local/share}"
user_bin="${HOME}/.local/bin"

cd "$repo_dir"
pnpm install --frozen-lockfile
pnpm --filter @passage/desktop tauri build --no-bundle

install -Dm755 src/desktop/src-tauri/target/release/passage_desktop "$user_bin/passage"
install -Dm644 src/desktop/src-tauri/icons/icon.png "$install_root/icons/hicolor/512x512/apps/nz.plu.passage.png"
install -Dm644 scripts/nz.plu.passage.desktop "$install_root/applications/nz.plu.passage.desktop"

printf 'Passage instalado em %s\n' "$user_bin/passage"
