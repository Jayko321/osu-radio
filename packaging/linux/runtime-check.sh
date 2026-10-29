#!/usr/bin/env bash
# Inside a disposable runtime-only Arch root, install and exercise the archive as a user.
# usage: runtime-check.sh ARCHIVE.tar.gz DIR_WITH_client.realm
# Offscreen only: native GUI, real-library import and audio acceptance belong to T10.
set -euo pipefail
export LC_ALL=C
archive=${1:?archive required} fixture=${2:?fixture directory required}
fail() { echo "FAIL: $*" >&2; exit 1; }
[[ -f /etc/arch-release && $(id -u) == 0 ]] || fail 'run inside a disposable Arch root as root'
for tool in cargo rustc gcc g++ clang; do ! command -v "$tool" >/dev/null || fail "$tool is present"; done
[[ -z $(dotnet --list-sdks) ]] || fail '.NET SDK is present'
[[ ! -e /src && ! -e /work && ! -e /build ]] || fail 'build tree is visible'

tmp=$(mktemp -d); chmod 755 "$tmp"
tar -xzf "$archive" -C "$tmp"
cp -a "$tmp"/osu-radio-*/usr/. /usr/
for b in /usr/bin/osu-radio-qt /usr/bin/osu-radio-server /usr/lib/osu-radio/osu-lazer-realm-parser /usr/lib/osu-radio/librealm-wrappers.so; do
  deps=$(ldd "$b")
  echo "$b: $deps"
  ! grep -q 'not found' <<<"$deps" || fail "unresolved libraries in $b"
done
echo 'ok: all ELF dependencies resolve using system packages'
pacman -Q
dotnet --list-runtimes

id smoke >/dev/null 2>&1 || useradd --create-home smoke
home=$tmp/home; install -d -o smoke -g smoke "$home"
clean_env=(setpriv --reuid=smoke --regid=smoke --init-groups env -i HOME="$home" PATH=/usr/bin:/bin LC_ALL=C)
fixture_hash=$(sha256sum "$fixture/client.realm" | cut -d' ' -f1)
server= gui= child=
cleanup() {
  for pid in "$gui" "$child" "$server"; do
    [[ -z $pid ]] || kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap cleanup EXIT
cd /
# An explicit empty runtime root must fail instead of falling back to build-host libraries.
mkdir "$tmp/no-dotnet"
if "${clean_env[@]}" DOTNET_ROOT="$tmp/no-dotnet" /usr/lib/osu-radio/osu-lazer-realm-parser "$fixture/client.realm" >"$tmp/missing-runtime.log" 2>&1; then
  fail 'helper succeeded without the required runtime'
fi
grep -q 'You must install .NET' "$tmp/missing-runtime.log" || { cat "$tmp/missing-runtime.log"; fail 'missing-runtime diagnostic'; }
echo 'ok: missing .NET runtime produces an explicit failure'

# Default SQLite placement and relative helper lookup, from an unrelated cwd without .env.
log=$tmp/server.log
"${clean_env[@]}" OSU_RADIO_SERVER_ADDRESS=127.0.0.1:0 /usr/bin/osu-radio-server >"$log" 2>&1 &
server=$!
for _ in $(seq 50); do grep -q 'listening on' "$log" && break; sleep 0.2; done
url=$(sed -n 's/.*listening on \(http[^ ]*\).*/\1/p' "$log" | head -1)
[[ -n $url ]] || { cat "$log" >&2; fail 'server did not start'; }
resp=$(curl -fsS -X POST "$url/api/user-data/osu-folders/metadata" -H 'content-type: application/json' \
  -d "{\"marker_path\":\"$fixture/client.realm\"}") || { cat "$log" >&2; fail 'helper read failed'; }
echo "metadata response: $resp"
[[ $resp == '{"beatmap_count":2}' ]] || fail 'expected the two beatmaps in full.realm'
kill "$server"; wait "$server" 2>/dev/null || true; server=
find "$home" -name '*.db' | grep -q . || fail 'SQLite database was not created under HOME'
[[ $(sha256sum "$fixture/client.realm" | cut -d' ' -f1) == "$fixture_hash" ]] || fail 'fixture changed'
echo 'ok: installed server read the Realm fixture through ../lib/osu-radio'

"${clean_env[@]}" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software /usr/bin/osu-radio >"$tmp/gui.log" 2>&1 &
gui=$!
for _ in $(seq 50); do child=$(pgrep -P "$gui" -f '^/usr/bin/osu-radio-server' || true); [[ -n $child ]] && break; sleep 0.2; done
kill -0 "$gui" 2>/dev/null || { cat "$tmp/gui.log" >&2; fail 'GUI exited early'; }
[[ -n $child ]] || { cat "$tmp/gui.log" >&2; fail 'GUI did not start its server'; }
kill "$gui"; wait "$gui" 2>/dev/null || true; gui=
# SIGTERM skips Rust Drop; normal window close is exercised by Qt probes and native acceptance.
echo 'ok: offscreen GUI started its child server (signal cleanup is test-owned)'
echo "runtime logs: $tmp"
