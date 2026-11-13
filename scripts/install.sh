#!/bin/sh
set -eu

if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
  echo 'Passage: this installer provides a binary only for Linux x86_64.' >&2
  exit 1
fi

for command_name in curl tar sha256sum install mktemp mkdir; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Passage: required command not found: $command_name" >&2
    exit 1
  fi
done

release_url="${PASSAGE_RELEASE_URL:-https://github.com/plu-nz/mono.passage/releases/latest/download}"
archive='passage-linux-x86_64.tar.gz'
temp_dir="$(mktemp -d)"
trap 'rm -rf -- "$temp_dir"' EXIT

curl -fsSL "$release_url/$archive" -o "$temp_dir/$archive"
curl -fsSL "$release_url/$archive.sha256" -o "$temp_dir/$archive.sha256"
(cd "$temp_dir" && sha256sum -c "$archive.sha256")
tar -xzf "$temp_dir/$archive" -C "$temp_dir"

extension_archive='passage-extension-chromium.tar.gz'
extension_available=false
if curl -fsSL "$release_url/$extension_archive" -o "$temp_dir/$extension_archive" 2>/dev/null; then
  curl -fsSL "$release_url/$extension_archive.sha256" -o "$temp_dir/$extension_archive.sha256"
  (cd "$temp_dir" && sha256sum -c "$extension_archive.sha256")
  extension_available=true
fi

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
if [ "$extension_available" = true ]; then
  extension_dir="$install_root/passage/extension-chromium"
  mkdir -p "$extension_dir"
  tar -xzf "$temp_dir/$extension_archive" -C "$extension_dir"
  printf 'Extension unpacked to %s; load this folder in Brave or Chrome and register the IDs with the connector.\n' "$extension_dir"
fi

host_registered=false
if [ -f "$temp_dir/passage-native-host" ] && [ -f "$temp_dir/install-native-host.sh" ]; then
  default_id=''
  if [ "$extension_available" = true ] && [ -f "$temp_dir/extension-id.txt" ]; then
    IFS= read -r default_id < "$temp_dir/extension-id.txt"
  fi
  brave_id="${PASSAGE_BRAVE_EXTENSION_ID:-$default_id}"
  chrome_id="${PASSAGE_CHROME_EXTENSION_ID:-$default_id}"
  if [ -n "$brave_id" ] && [ -n "$chrome_id" ]; then
    "$temp_dir/install-native-host.sh" "$user_bin/passage-native-host" "$brave_id" "$chrome_id"
    host_registered=true
  fi
fi

printf 'Passage installed at %s\n' "$user_bin/passage"
if [ -f "$temp_dir/passage-native-host" ] && [ "$host_registered" = false ]; then
  printf 'Connector installed. To register Brave and Chrome, run: %s/passage-install-native-host %s/passage-native-host ID_BRAVE ID_CHROME\n' "$user_bin" "$user_bin"
fi
