#!/usr/bin/env bash
set -euo pipefail

host="${RBE_HOST:-root@2.28.55.172}"
remote_flake_dir="${RBE_FLAKE_DIR:-/opt/neo-rbe-nix}"
remote_profile="${RBE_NIX_PROFILE:-/opt/neo-nix}"
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

ssh "$host" "install -d -m 0755 '$remote_flake_dir'"
scp "$script_dir/flake.nix" "$script_dir/flake.lock" "$host:$remote_flake_dir/"
ssh "$host" \
  "cd '$remote_flake_dir' &&
   nix build --no-update-lock-file --profile '$remote_profile' .#neo-rbe-env &&
   '$remote_profile/bin/rustc' --version &&
   PKG_CONFIG_PATH='$remote_profile/lib/pkgconfig:$remote_profile/share/pkgconfig' \
     '$remote_profile/bin/pkg-config' --modversion gtk4 libadwaita-1 sqlite3"
