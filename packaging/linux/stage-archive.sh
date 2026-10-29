#!/usr/bin/env bash
# Assemble the system-layout Linux archive from build.sh output (no second Rust build).
# usage: packaging/linux/stage-archive.sh WORKDIR [OUTDIR]   (OUTDIR defaults to WORKDIR/dist)
# Run from any host with Bash, coreutils, tar, gzip and binutils; the build tree itself is not needed.
set -euo pipefail
export LC_ALL=C

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
work=$(cd "$1" && pwd)
dist=$(mkdir -p "${2:-$work/dist}" && cd "${2:-$work/dist}" && pwd)
in=$work/out
version=0.1.0-alpha
name=osu-radio-$version-arch-x86_64

for f in osu-radio-qt osu-radio-server helper/osu-lazer-realm-parser helper/osu-lazer-realm-parser.dll helper/osu-lazer-realm-parser.deps.json \
  helper/osu-lazer-realm-parser.runtimeconfig.json helper/Realm.dll helper/librealm-wrappers.so \
  source-info.txt source-files.sha256; do
  [[ -f $in/$f ]] || { echo "missing build output: $in/$f (run build.sh first)" >&2; exit 1; }
done
[[ -s $in/build-env.txt ]] || { echo "missing $in/build-env.txt (run build.sh first)" >&2; exit 1; }
if [[ -n $(find "$in/helper" -name 'libcoreclr.so' -print -quit) ]]; then
  echo 'helper publish is self-contained; the archive needs the framework-dependent one' >&2; exit 1
fi
# The installed inventory must describe this publish, not an earlier candidate.
while IFS=$'\t' read -r file size hash rest; do
  [[ $file == filename ]] && continue
  [[ -f $in/helper/$file ]] && [[ $(stat -c %s "$in/helper/$file") == "$size" ]] &&
    [[ $(sha256sum "$in/helper/$file" | cut -d' ' -f1) == "$hash" ]] || {
      echo "helper inventory mismatch: $file (refresh HELPER-FILES.tsv)" >&2; exit 1;
    }
done < "$repo/packaging/arch/licenses/HELPER-FILES.tsv"
[[ $(find "$in/helper" -type f | wc -l) == $(tail -n +2 "$repo/packaging/arch/licenses/HELPER-FILES.tsv" | wc -l) ]] || {
  echo 'helper inventory has a different file count' >&2; exit 1;
}

stage=$dist/stage/$name
rm -rf "$dist/stage"
install -d "$stage/usr/bin" "$stage/usr/lib/osu-radio" "$stage/usr/share/applications" \
  "$stage/usr/share/licenses/osu-radio"
strip --strip-unneeded -o "$stage/usr/bin/osu-radio-qt" "$in/osu-radio-qt"
strip --strip-unneeded -o "$stage/usr/bin/osu-radio-server" "$in/osu-radio-server"
chmod 755 "$stage/usr/bin/osu-radio-qt" "$stage/usr/bin/osu-radio-server"
ln -s osu-radio-qt "$stage/usr/bin/osu-radio"
cp -r "$in/helper/." "$stage/usr/lib/osu-radio/"
# NuGet files arrive owner-only executable; make the modes explicit.
find "$stage/usr/lib/osu-radio" -type f -exec chmod 644 {} +
chmod 755 "$stage/usr/lib/osu-radio/osu-lazer-realm-parser" "$stage/usr/lib/osu-radio/librealm-wrappers.so"
install -m644 "$repo/packaging/linux/osu-radio.desktop" "$stage/usr/share/applications/"
cp -r "$repo/packaging/icons/hicolor" "$stage/usr/share/icons-tmp" && mkdir -p "$stage/usr/share/icons" \
  && mv "$stage/usr/share/icons-tmp" "$stage/usr/share/icons/hicolor"
install -m644 "$repo/LICENSE" "$stage/usr/share/licenses/osu-radio/LICENSE"
install -m644 "$repo"/packaging/arch/licenses/* "$stage/usr/share/licenses/osu-radio/"
install -m644 "$repo/packaging/linux/ARCHIVE-README.md" "$stage/README.md"

# Provenance: commit, uncommitted changes (a candidate probe, never a release), toolchains, hashes.
{
  echo "osu-radio $version Arch x86_64 system archive"
  cat "$in/source-info.txt"
  echo
  cat "$in/build-env.txt"
  echo
  echo "packaging inputs at staging (may be newer than the binary source snapshot):"
  (cd "$repo"; find packaging/linux packaging/icons packaging/arch/licenses -type f -print0 |
    sort -z | xargs -0 sha256sum | sed 's/^/  /')
  echo
  echo "unstripped Rust build inputs:"
  (cd "$in" && sha256sum osu-radio-qt osu-radio-server | sed 's/^/  /')
  echo "installed (stripped) executables:"
  (cd "$stage/usr/bin" && sha256sum osu-radio-qt osu-radio-server | sed 's/^/  /')
  echo "helper publish files:"
  (cd "$stage/usr/lib/osu-radio" && find . -type f | LC_ALL=C sort | xargs sha256sum | sed 's/^/  /')
} > "$stage/BUILD-INFO.txt"

# Dynamic dependencies actually required by the binaries (not masked by the build host).
{
  for b in osu-radio-qt osu-radio-server; do
    echo "$b NEEDED:"; readelf -d "$stage/usr/bin/$b" | awk '/NEEDED|RUNPATH|RPATH/ {print "  "$0}'
  done
  echo "highest GLIBC symbol versions:"
  for b in osu-radio-qt osu-radio-server; do
    printf '  %s: ' "$b"
    readelf --dyn-syms -W "$stage/usr/bin/$b" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1
  done
  printf '  osu-radio-qt libstdc++: '
  readelf --dyn-syms -W "$stage/usr/bin/osu-radio-qt" | grep -o 'GLIBCXX_[0-9.]*' | sort -uV | tail -1
  printf '  native Realm library: '
  readelf --dyn-syms -W "$stage/usr/lib/osu-radio/librealm-wrappers.so" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1
  echo "native Realm NEEDED:"; readelf -d "$stage/usr/lib/osu-radio/librealm-wrappers.so" | awk '/NEEDED/ {print "  "$0}'
} >> "$stage/BUILD-INFO.txt"

install -m644 "$in/source-files.sha256" "$stage/SOURCE-FILES.sha256"
epoch=$(sed -n 's/^source epoch: //p' "$in/source-info.txt")
[[ $epoch =~ ^[0-9]+$ ]] || { echo 'missing source epoch' >&2; exit 1; }
archive=$dist/$name.tar.gz
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$epoch" -C "$dist/stage" -cf - "$name" \
  | gzip -9n > "$archive"
(cd "$dist" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")

# Self-check: the archive lists everything the installed layout and its contract need.
listing=$(tar -tzf "$archive")
for p in usr/bin/osu-radio-qt usr/bin/osu-radio usr/bin/osu-radio-server usr/lib/osu-radio/osu-lazer-realm-parser \
  usr/lib/osu-radio/osu-lazer-realm-parser.dll usr/lib/osu-radio/librealm-wrappers.so usr/lib/osu-radio/Realm.dll \
  usr/share/applications/osu-radio.desktop usr/share/icons/hicolor/256x256/apps/osu-radio.png \
  usr/share/licenses/osu-radio/LICENSE usr/share/licenses/osu-radio/RUST-NOTICES.txt BUILD-INFO.txt SOURCE-FILES.sha256 README.md; do
  grep -qxF "$name/$p" <<<"$listing" || { echo "archive is missing $p" >&2; exit 1; }
done
if grep -qE '(^|/)(Cargo\.|src/|target/|obj/)|\.rs$|\.cs$' <<<"$listing"; then
  echo 'archive contains sources or build-tree files' >&2; exit 1
fi
echo "$archive"; cat "$dist/$name.tar.gz.sha256"
