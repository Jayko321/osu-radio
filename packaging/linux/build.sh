#!/usr/bin/env bash
# Build in a prepared, isolated Arch x86_64 root with system Qt and .NET 10.
# usage: packaging/linux/build.sh WORKDIR
# WORKDIR holds private caches, targets and outputs; the checkout is read-only.
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
work=$(mkdir -p "${1:?usage: build.sh WORKDIR}" && cd "$1" && pwd)
[[ $(uname -m) == x86_64 ]] || { echo 'x86_64 is required' >&2; exit 1; }
. /etc/os-release
[[ $ID == arch ]] || { echo "Arch Linux is required, found $PRETTY_NAME" >&2; exit 1; }

export RUSTUP_HOME=$work/rustup CARGO_HOME=$work/cargo
export DOTNET_CLI_HOME=$work/dotnet-home DOTNET_NOLOGO=1 DOTNET_CLI_TELEMETRY_OPTOUT=1
export NUGET_PACKAGES=$work/nuget NUGET_HTTP_CACHE_PATH=$work/nuget-http
unset RUSTUP_TOOLCHAIN CARGO_ENCODED_RUSTFLAGS
mkdir -p "$work/out"
# Capture the inputs at build time, not later when the archive is staged.
{
  echo "source commit: $(git -C "$repo" rev-parse HEAD)"
  echo "source epoch: $(git -C "$repo" log -1 --format=%ct)"
  if [[ -n $(git -C "$repo" status --porcelain) ]]; then
    echo 'working tree: DIRTY - candidate probe, not a release'
    echo "uncommitted diff sha256: $(git -C "$repo" diff HEAD | sha256sum | cut -d' ' -f1)"
    git -C "$repo" status --porcelain --untracked-files=all
  else
    echo 'working tree: clean'
  fi
  echo "Cargo.lock sha256: $(sha256sum "$repo/Cargo.lock" | cut -d' ' -f1)"
} > "$work/out/source-info.txt"
(cd "$repo"; git ls-files --cached --others --exclude-standard -z | sort -zu |
  while IFS= read -r -d '' f; do [[ ! -f $f ]] || sha256sum "$f"; done) > "$work/out/source-files.sha256"

# Framework-dependent helper: runs on the consumer's system .NET 10 runtime, no SDK needed.
# Published from a copy of its three source files so stray bin/obj dirs of the checkout cannot leak in.
helper_src=$repo/tools/osu-lazer-realm-parser
rm -rf "$work/helper-src" "$work/out/helper"
mkdir -p "$work/helper-src"
cp "$helper_src/osu-lazer-realm-parser.csproj" "$helper_src/Program.cs" "$helper_src/FodyWeavers.xml" "$work/helper-src/"
sdk=$(dotnet --list-sdks | awk '/^10\./ { version=$1 } END { print version }')
[[ -n $sdk ]] || { echo 'The dotnet-sdk-10.0 build dependency is required' >&2; exit 1; }
printf '{"sdk":{"version":"%s","rollForward":"latestPatch"}}\n' "$sdk" > "$work/helper-src/global.json"
(cd "$work/helper-src"; dotnet publish --configuration Release \
  --runtime linux-x64 --self-contained false --output "$work/out/helper" --nologo)

# Rust apps. Defaults keep SQLite + API docs (no PostgreSQL), as in the Arch recipe.
cd "$repo"
export CARGO_TARGET_DIR=$work/target
export QMAKE=/usr/bin/qmake6
# The installed path is the contract with the system layout; the helper is also found relative
# to the executable (../lib/osu-radio). Build-tree and cargo-home prefixes are remapped out of
# the binaries so no build path can be mistaken for a runtime one.
export OSU_LAZER_REALM_PARSER_PATH=/usr/lib/osu-radio/osu-lazer-realm-parser
# GNU ld drops the cxx-qt static initializers; the Arch recipe also links with LLD.
export RUSTFLAGS="-C link-arg=-fuse-ld=lld --remap-path-prefix=$repo=/build/osu-radio --remap-path-prefix=$work=/build/work --remap-path-prefix=$HOME=/build/home"
# Same for C++ (Qt headers, cxx-qt generated code) so assertion strings carry no build paths.
export CFLAGS="-ffile-prefix-map=$repo=/build/osu-radio -ffile-prefix-map=$work=/build/work"
export CXXFLAGS=$CFLAGS
rustup toolchain install --no-self-update
cargo build --release --locked -p osu-radio-qt -p osu-radio-server

install -m755 "$work/target/release/osu-radio-qt" "$work/target/release/osu-radio-server" "$work/out/"
{
  echo "build environment:"
  echo "  $PRETTY_NAME, $(uname -m), glibc $(ldd --version | head -1 | awk '{print $NF}')"
  echo "  $(rustc -V), $(cargo -V)"
  echo "  .NET SDK $sdk; $(dotnet --list-runtimes)"
  echo "  system Qt $($QMAKE -query QT_VERSION)"
  echo "  gcc $(gcc -dumpfullversion), $(ld.lld --version | head -1)"
  echo "  Arch package state (rolling; captured at build time):"
  pacman -Q
  echo "  package database SHA-256:"
  sha256sum /var/lib/pacman/sync/*.db
  echo "  RUSTFLAGS: $RUSTFLAGS"
  echo "  CFLAGS/CXXFLAGS: $CFLAGS"
  echo "  cargo build --release --locked -p osu-radio-qt -p osu-radio-server (default features: SQLite + API docs)"
  echo "  helper: dotnet publish --configuration Release --runtime linux-x64 --self-contained false"
  echo "  OSU_LAZER_REALM_PARSER_PATH=$OSU_LAZER_REALM_PARSER_PATH (fallback only; ../lib/osu-radio is found first)"
} > "$work/out/build-env.txt"
echo "build output: $work/out"
