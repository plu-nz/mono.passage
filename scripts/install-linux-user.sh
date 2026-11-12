#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
install_root="${XDG_DATA_HOME:-$HOME/.local/share}"
user_bin="${HOME}/.local/bin"
build_tmp="${XDG_CACHE_HOME:-$HOME/.cache}/passage-go-tmp"
mkdir -p "$build_tmp"
export GOTMPDIR="$build_tmp" TMPDIR="$build_tmp"

cd "$repo_dir"
pnpm install --frozen-lockfile
pnpm --filter @passage/desktop build
pnpm build:extension
go build -tags gtk3 -o src/desktop/build/passage ./src/desktop
go build -o src/native-host/build/passage-native-host ./src/native-host

install -Dm755 src/desktop/build/passage "$user_bin/passage"
install -Dm755 src/native-host/build/passage-native-host "$user_bin/passage-native-host"
install -Dm755 scripts/install-native-host.sh "$user_bin/passage-install-native-host"
extension_dir="$install_root/passage/extension-chromium"
mkdir -p "$extension_dir"
cp -a src/extension/.output/chrome-mv3/. "$extension_dir/"
extension_id="$(python3 - <<'PY'
import base64
import hashlib
import json
from pathlib import Path
identity = json.loads(Path('src/extension/identity.json').read_text(encoding='utf-8'))
manifest = json.loads(Path('src/extension/.output/chrome-mv3/manifest.json').read_text(encoding='utf-8'))
if manifest.get('key') != identity['key']:
    raise SystemExit('Chave da extensão não corresponde ao ID configurado')
digest = hashlib.sha256(base64.b64decode(identity['key'], validate=True)).digest()
extension_id = ''.join(chr(97 + nibble) for byte in digest[:16] for nibble in (byte >> 4, byte & 15))
if extension_id != identity['id']:
    raise SystemExit('ID da extensão inválido')
print(extension_id)
PY
)"
"$user_bin/passage-install-native-host" "$user_bin/passage-native-host" "$extension_id" "$extension_id"
install -Dm644 src/desktop/icons/icon.png "$install_root/icons/hicolor/512x512/apps/nz.plu.passage.png"
install -Dm644 scripts/nz.plu.passage.desktop "$install_root/applications/nz.plu.passage.desktop"

printf 'Passage instalado em %s\n' "$user_bin/passage"
printf 'Extensão instalada em %s; carregue essa pasta em Brave e Chrome e aprove cada pareamento.\n' "$extension_dir"
