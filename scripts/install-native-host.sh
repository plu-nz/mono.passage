#!/usr/bin/env sh
set -eu

if [ "$#" -ne 3 ]; then
  echo 'Uso: scripts/install-native-host.sh CAMINHO_DO_HOST ID_BRAVE ID_CHROME' >&2
  exit 2
fi

host_path=$1
brave_id=$2
chrome_id=$3

if [ ! -x "$host_path" ]; then
  echo 'O host precisa existir e ser executável.' >&2
  exit 2
fi

python3 - "$host_path" "$brave_id" "$chrome_id" <<'PY'
import json
import os
import pathlib
import re
import sys

host, brave_id, chrome_id = sys.argv[1:]
for extension_id in (brave_id, chrome_id):
    if not re.fullmatch(r"[a-p]{32}", extension_id):
        raise SystemExit("ID de extensão inválido; consulte brave://extensions e chrome://extensions")

config_home = pathlib.Path(os.environ.get("XDG_CONFIG_HOME", pathlib.Path.home() / ".config"))
host = str(pathlib.Path(host).resolve(strict=True))
profiles = [("BraveSoftware/Brave-Browser", brave_id), ("google-chrome", chrome_id)]
for variant in ("Brave-Origin", "Brave-Origin-Beta", "Brave-Origin-Nightly", "Brave-Browser-Beta", "Brave-Browser-Nightly"):
    if (config_home / "BraveSoftware" / variant).is_dir():
        profiles.append((f"BraveSoftware/{variant}", brave_id))

for profile, extension_id in profiles:
    directory = config_home / profile / "NativeMessagingHosts"
    directory.mkdir(parents=True, exist_ok=True)
    manifest = directory / "com.plunz.passage.json"
    if manifest.is_symlink():
        raise SystemExit(f"Manifesto simbólico recusado: {manifest}")
    data = {
        "name": "com.plunz.passage",
        "description": "Passage native messaging host",
        "path": host,
        "type": "stdio",
        "allowed_origins": [f"chrome-extension://{extension_id}/"],
    }
    temporary = manifest.with_suffix(".json.tmp")
    with temporary.open("w", encoding="utf-8") as stream:
        json.dump(data, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    temporary.chmod(0o600)
    temporary.replace(manifest)
    print(manifest)
PY
