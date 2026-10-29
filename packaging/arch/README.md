# Arch Linux packages

The first release has two mutually exclusive x86_64 packages:

- [`osu-radio-git`](osu-radio-git/PKGBUILD) builds the full `main` Git history
  with system Qt, .NET 10 and the Rust toolchain in `rust-toolchain.toml`.
- [`osu-radio-bin`](osu-radio-bin/PKGBUILD) installs the separate Arch system
  archive, using system Qt and .NET 10 runtime. It does not need Rust, a C++
  compiler or the .NET SDK at installation time.

Both provide and conflict with `osu-radio`, keep the commands `osu-radio`,
`osu-radio-qt`, `osu-radio-server`, and find the framework-dependent helper in
`/usr/lib/osu-radio`. They install the desktop entry, hicolor icons and notices.
The former root [`PKGBUILD`](PKGBUILD) and [`.SRCINFO`](.SRCINFO) mirror the
`-git` recipe for existing references; use the two dedicated directories for
packaging. The obsolete .NET 8 `PKGBUILD.local` was removed.

## Release staging

The binary recipe is **not yet releasable**: the existing `v0.1.0-alpha` tag
points at old code and its all-zero checksum intentionally rejects any archive.
T09 must build the Arch archive from the selected clean release commit, after
separate authorization to move the tag publish the matching asset at the recipe
URL, set its real SHA-256, and regenerate `.SRCINFO` in release staging. The
partial T09 [staging script](../release/stage-arch-recipes.py) reads both
recipes from the selected commit, checks the archive's recorded commit and clean
status, and generates the recipes in a new directory using `makepkg`:

```sh
python3 packaging/release/stage-arch-recipes.py /path/to/osu-radio-0.1.0-alpha-arch-x86_64.tar.gz FULL_COMMIT /path/to/new-staging
```

This checks recorded provenance; it does not verify the remote tag, artifact
contents or release readiness. Run it in Arch with `makepkg` installed. Never
substitute the T05 dirty-tree candidate hash for a release asset. Validate the
final `-git` main and `-bin` asset again after staging. No tag, GitHub release
or AUR publication is performed by these recipes.

The VCS recipe uses `0.1.0alpha.r<commit-count>.g<hash>` from the complete
main history. Its VCS source alone uses `SKIP`; its desktop entry, icons and
notices are tracked within that Git revision. `-bin` verifies the whole archive
including those files. `pkgrel=2` upgrades the prior `0.1.0alpha-1` binary.

## Build and install

In a clean Arch build environment with `base-devel`, run as an ordinary user:

```sh
cd packaging/arch/osu-radio-git  # or osu-radio-bin, once release staging is complete
makepkg --printsrcinfo > .SRCINFO
makepkg -si
```

Choose a desktop portal implementation appropriate to the session when pacman
requests one. osu!/Wine are optional; a manually installed osu! library works.
Both variants share the existing XDG SQLite database and Qt settings; neither
contains hooks to reset or remove user data. Pacman will report a conflict if
both are requested. To switch, allow pacman to replace the installed variant;
only compatible schema transitions are supported. Arbitrary database downgrades
are not supported. Removal uses `sudo pacman -R osu-radio-git` or
`sudo pacman -R osu-radio-bin` and leaves user data in place.

A local test of `-bin` must use a separate recipe with a local immutable candidate
archive and its actual checksum; never change the release checksum to a candidate.
See [T07](../../docs/impl/first-release-packaging/T07.md) for performed checks,
limits and the final inputs required by T09. Native desktop, audio and lifecycle
acceptance remain separate. [T05](../../docs/impl/first-release-packaging/T05.md)
records the Arch archive provenance and open Qt probe failure.
