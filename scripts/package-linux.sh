#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="${1:-$repo_dir/dist/release}"
archive='passage-linux-x86_64.tar.gz'
extension_archive='passage-extension-chromium.tar.gz'
extension_dir="$repo_dir/extension/.output/chrome-mv3"

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo 'Packaging is available only on Linux x86_64.' >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo 'Python 3 is required to package the extension identity.' >&2
  exit 1
fi
if [[ ! -f "$extension_dir/manifest.json" ]]; then
  echo 'Built extension not found; run pnpm build:extension.' >&2
  exit 1
fi

mkdir -p "$output_dir"
stage_parent="${TMPDIR:-${XDG_CACHE_HOME:-$HOME/.cache}}"
mkdir -p "$stage_parent"
stage_dir="$(mktemp -d -p "$stage_parent" passage-package.XXXXXX)"
trap 'rm -rf -- "$stage_dir"' EXIT

desktop_binary="${PASSAGE_DESKTOP_BINARY:-$repo_dir/desktop/build/passage}"
install -Dm755 "$desktop_binary" "$stage_dir/passage"
host_binary="${PASSAGE_NATIVE_HOST_BINARY:-$repo_dir/native-host/build/passage-native-host}"
install -Dm755 "$host_binary" "$stage_dir/passage-native-host"
install -Dm755 "$repo_dir/scripts/install-native-host.sh" "$stage_dir/install-native-host.sh"
python3 - "$repo_dir/extension/identity.json" "$extension_dir/manifest.json" "$stage_dir/extension-id.txt" <<'PY'
import base64
import hashlib
import json
import pathlib
import sys

identity = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
manifest = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))
if manifest.get("key") != identity["key"]:
    raise SystemExit("Extension public key does not match the release identity")
digest = hashlib.sha256(base64.b64decode(identity["key"], validate=True)).digest()
extension_id = "".join(chr(97 + nibble) for byte in digest[:16] for nibble in (byte >> 4, byte & 15))
if extension_id != identity["id"]:
    raise SystemExit("Extension ID does not match the public key")
pathlib.Path(sys.argv[3]).write_text(extension_id + "\n", encoding="utf-8")
PY
install -Dm644 "$repo_dir/desktop/icons/icon.png" "$stage_dir/nz.plu.passage.png"
install -Dm644 "$repo_dir/scripts/nz.plu.passage.desktop" "$stage_dir/nz.plu.passage.desktop"

tar -C "$stage_dir" -czf "$output_dir/$archive" passage passage-native-host install-native-host.sh extension-id.txt nz.plu.passage.png nz.plu.passage.desktop
(cd "$output_dir" && sha256sum "$archive" > "$archive.sha256")
tar -C "$extension_dir" -czf "$output_dir/$extension_archive" .
(cd "$output_dir" && sha256sum "$extension_archive" > "$extension_archive.sha256")
printf 'Pacote criado em %s/%s\n' "$output_dir" "$archive"
