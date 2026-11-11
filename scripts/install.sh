#!/bin/sh
set -eu

if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
  echo 'Passage: este instalador oferece binário apenas para Linux x86_64.' >&2
  exit 1
fi

for command_name in curl tar sha256sum install mktemp; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Passage: comando necessário não encontrado: $command_name" >&2
    exit 1
  fi
done

release_url='https://github.com/plu-nz/passage/releases/latest/download'
archive='passage-linux-x86_64.tar.gz'
temp_dir="$(mktemp -d)"
trap 'rm -rf -- "$temp_dir"' EXIT

curl -fsSL "$release_url/$archive" -o "$temp_dir/$archive"
curl -fsSL "$release_url/$archive.sha256" -o "$temp_dir/$archive.sha256"
(cd "$temp_dir" && sha256sum -c "$archive.sha256")
tar -xzf "$temp_dir/$archive" -C "$temp_dir"

install_root="${XDG_DATA_HOME:-$HOME/.local/share}"
user_bin="$HOME/.local/bin"
install -Dm755 "$temp_dir/passage" "$user_bin/.passage.new"
mv -f "$user_bin/.passage.new" "$user_bin/passage"
if [ -f "$temp_dir/passage-native-host" ]; then
  install -Dm755 "$temp_dir/passage-native-host" "$user_bin/.passage-native-host.new"
  mv -f "$user_bin/.passage-native-host.new" "$user_bin/passage-native-host"
fi
if [ -f "$temp_dir/install-native-host.sh" ]; then
  install -Dm755 "$temp_dir/install-native-host.sh" "$user_bin/passage-install-native-host"
fi
install -Dm644 "$temp_dir/nz.plu.passage.png" "$install_root/icons/hicolor/512x512/apps/nz.plu.passage.png"
install -Dm644 "$temp_dir/nz.plu.passage.desktop" "$install_root/applications/nz.plu.passage.desktop"

if [ -f "$temp_dir/passage-native-host" ] && [ -n "${PASSAGE_BRAVE_EXTENSION_ID:-}" ] && [ -n "${PASSAGE_CHROME_EXTENSION_ID:-}" ]; then
  if [ -f "$temp_dir/install-native-host.sh" ]; then
    "$temp_dir/install-native-host.sh" "$user_bin/passage-native-host" "$PASSAGE_BRAVE_EXTENSION_ID" "$PASSAGE_CHROME_EXTENSION_ID"
  fi
fi

printf 'Passage instalado em %s\n' "$user_bin/passage"
if [ -f "$temp_dir/passage-native-host" ] && { [ -z "${PASSAGE_BRAVE_EXTENSION_ID:-}" ] || [ -z "${PASSAGE_CHROME_EXTENSION_ID:-}" ]; }; then
  printf 'Conector instalado. Para registrar Brave e Chrome, execute: %s/passage-install-native-host %s/passage-native-host ID_BRAVE ID_CHROME\n' "$user_bin" "$user_bin"
fi
