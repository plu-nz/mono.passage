#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="${1:-$repo_dir/dist/release}"
archive='passage-linux-x86_64.tar.gz'
extension_archive='passage-extension-chromium.tar.gz'
extension_dir="$repo_dir/src/extension/.output/chrome-mv3"

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo 'Empacotamento disponível apenas em Linux x86_64.' >&2
  exit 1
fi
if [[ ! -f "$extension_dir/manifest.json" ]]; then
  echo 'Extensão compilada não encontrada; execute pnpm build:extension.' >&2
  exit 1
fi

mkdir -p "$output_dir"
stage_dir="$(mktemp -d)"
trap 'rm -rf -- "$stage_dir"' EXIT

desktop_binary="${PASSAGE_DESKTOP_BINARY:-$repo_dir/src/desktop/src-tauri/target/release/passage_desktop}"
install -Dm755 "$desktop_binary" "$stage_dir/passage"
host_binary="${PASSAGE_NATIVE_HOST_BINARY:-$repo_dir/src/native-host/target/release/passage-native-host}"
install -Dm755 "$host_binary" "$stage_dir/passage-native-host"
install -Dm755 "$repo_dir/scripts/install-native-host.sh" "$stage_dir/install-native-host.sh"
install -Dm644 "$repo_dir/src/desktop/src-tauri/icons/icon.png" "$stage_dir/nz.plu.passage.png"
install -Dm644 "$repo_dir/scripts/nz.plu.passage.desktop" "$stage_dir/nz.plu.passage.desktop"

tar -C "$stage_dir" -czf "$output_dir/$archive" passage passage-native-host install-native-host.sh nz.plu.passage.png nz.plu.passage.desktop
(cd "$output_dir" && sha256sum "$archive" > "$archive.sha256")
tar -C "$extension_dir" -czf "$output_dir/$extension_archive" .
(cd "$output_dir" && sha256sum "$extension_archive" > "$extension_archive.sha256")
printf 'Pacote criado em %s/%s\n' "$output_dir" "$archive"
