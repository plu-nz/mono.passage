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
install -Dm644 "$temp_dir/nz.plu.passage.png" "$install_root/icons/hicolor/512x512/apps/nz.plu.passage.png"
install -Dm644 "$temp_dir/nz.plu.passage.desktop" "$install_root/applications/nz.plu.passage.desktop"

printf 'Passage instalado em %s\n' "$user_bin/passage"
