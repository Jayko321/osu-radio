#!/usr/bin/env bash
# Run a command as root inside an unprivileged Ubuntu 22.04 base rootfs (no sudo, no container engine).
# usage: enter-ubuntu.sh ROOTFS [--bind HOST:GUEST]... -- COMMAND [ARG...]
# Needs util-linux unshare with --map-auto and subuid/subgid ranges for the calling user.
set -euo pipefail
root=$(realpath "$1"); shift
binds=()
while [[ ${1:-} == --bind ]]; do binds+=("$2"); shift 2; done
[[ ${1:-} == -- ]] && shift
export root
export BINDS="${binds[*]:-}"
exec unshare --user --map-root-user --map-auto --mount --pid --fork --kill-child -- bash -c '
  set -e
  mount --rbind /dev "$root/dev"; mount --make-rslave "$root/dev"
  mount -t proc proc "$root/proc"
  mount --rbind /sys "$root/sys" 2>/dev/null || true
  for b in $BINDS; do mkdir -p "$root/${b#*:}"; mount --rbind "${b%%:*}" "$root/${b#*:}"; done
  cp /etc/resolv.conf "$root/etc/resolv.conf"
  exec chroot "$root" /usr/bin/env -i HOME=/root TERM=dumb LANG=C.UTF-8 \
    PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin "$@"
' bash "$@"
