#!/usr/bin/env bash
# Run only inside the pinned disposable Arch CI container, as root.
set -euo pipefail
[[ -f /.dockerenv && $(id -u) == 0 ]] || { echo 'run in the disposable Arch CI container as root' >&2; exit 1; }
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
sha=${1:?full source SHA required}
# Keep the helper's build path identical to the reviewed T05 publish inventory.
work=/work
python3 -B "$repo/packaging/release/stage.py" check "$sha"
snapshot=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["arch_snapshot"])' "$repo/packaging/release/inputs.json")
printf 'Server = https://archive.archlinux.org/repos/%s/$repo/os/$arch\n' "$snapshot" > /etc/pacman.d/mirrorlist
pacman -Syyuu --noconfirm --needed base-devel rustup dotnet-sdk-10.0 \
  qt6-base qt6-declarative qt6-svg qt6-wayland qt6-shadertools alsa-lib lld clang git python curl
bash "$repo/packaging/linux/build.sh" "$work"
bash "$repo/packaging/linux/stage-archive.sh" "$work"
python3 -B "$repo/packaging/release/stage.py" check "$sha"
