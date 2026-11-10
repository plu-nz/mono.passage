#!/bin/sh
set -eu

if [ "$(uname -s)" != Linux ]; then
  echo 'Passage: o instalador atual suporta apenas Linux.' >&2
  exit 1
fi

for command_name in git bash pnpm cargo; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Passage: comando necessário não encontrado: $command_name" >&2
    exit 1
  fi
done

temp_dir="$(mktemp -d)"
trap 'rm -rf -- "$temp_dir"' EXIT

git clone --quiet --depth 1 https://github.com/plu-nz/passage.git "$temp_dir/passage"
bash "$temp_dir/passage/scripts/install-linux-user.sh"
