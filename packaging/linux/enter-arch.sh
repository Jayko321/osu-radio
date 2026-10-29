#!/usr/bin/env bash
# Run in a prepared Arch root without changing the host (requires subuid/subgid).
# usage: enter-arch.sh ROOTFS [--bind HOST:GUEST]... -- COMMAND [ARG...]
set -euo pipefail
root=$(realpath "${1:?rootfs required}"); shift
exec unshare --user --map-root-user --map-auto --mount --pid --fork --kill-child -- bash -c '
  set -euo pipefail
  root=$1; shift
  mount --rbind /dev "$root/dev"; mount --make-rslave "$root/dev"
  mount -t proc proc "$root/proc"
  while [[ ${1:-} == --bind ]]; do
    binding=$2; shift 2
    mkdir -p "$root/${binding#*:}"
    mount --rbind "${binding%%:*}" "$root/${binding#*:}"
    mount --make-rslave "$root/${binding#*:}"
  done
  [[ ${1:-} == -- ]] && shift
  cp /etc/resolv.conf "$root/etc/resolv.conf"
  exec chroot "$root" /usr/bin/env -i HOME=/root TERM=dumb LANG=C.UTF-8 \
    PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin "$@"
' bash "$root" "$@"
