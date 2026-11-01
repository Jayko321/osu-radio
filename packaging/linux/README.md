# Arch build and system archive

Builds `osu-radio-qt`, `osu-radio-server` and the framework-dependent .NET 10
lazer helper in a clean **Arch x86_64** root with **system Qt**. The archive feeds
`osu-radio-bin` (T07); package operations and native import/audio acceptance are
separate checks. Ubuntu and AppImage are outside the first release.

| File | Role |
| --- | --- |
| [build.sh](build.sh) | Builds in a prepared root; writes `WORKDIR/out/{osu-radio-qt,osu-radio-server,helper/,build-env.txt,source-info.txt,source-files.sha256}`. |
| [stage-archive.sh](stage-archive.sh) | Creates `osu-radio-<version>-arch-x86_64.tar.gz` and `.sha256`; checks composition and rejects incomplete/self-contained helper output. |
| [runtime-check.sh](runtime-check.sh) | Installs into a disposable Arch runtime root; runs server/helper/offscreen GUI as an ordinary user, with a missing-.NET negative control. |
| [enter-arch.sh](enter-arch.sh) | Enters a prepared root through `unshare` (requires subuid/subgid ranges). Only explicitly bound paths are visible. |
| [ARCHIVE-README.md](ARCHIVE-README.md), [osu-radio.desktop](osu-radio.desktop) | Installed instructions and desktop entry. |
| [pins.env](pins.env), [enter-ubuntu.sh](enter-ubuntu.sh) | Historical Ubuntu inputs only; unused by the Arch build. |

From an Arch host with `arch-install-scripts`, use a fresh root and private
work directory. [pacstrap -N](https://man.archlinux.org/man/pacstrap.8.en) supports
unprivileged namespaces. Its default keyring/mirrorlist comes from the host;
use `-C` for a private pacman configuration/cache when needed. Do not run a
partial system upgrade to prepare a build.

```sh
mkdir -p target/arch-build/work
pacstrap -N target/arch-build/root base base-devel rustup dotnet-sdk-10.0 \
  qt6-base qt6-declarative qt6-svg qt6-wayland qt6-shadertools alsa-lib \
  lld clang git python curl ripgrep
packaging/linux/enter-arch.sh target/arch-build/root \
  --bind "$PWD:/src" --bind "$PWD/target/arch-build/work:/work" -- \
  /src/packaging/linux/build.sh /work
packaging/linux/stage-archive.sh target/arch-build/work
```

Use an immutable checkout/snapshot while building. Rust is selected by
`rust-toolchain.toml`, SQLite + API docs stay enabled, and `Cargo.lock` is locked.
The .NET 10 SDK is selected explicitly. All Cargo/NuGet caches and build outputs
are private to `WORKDIR`; no downloaded Qt or .NET SDK is used. Rust and C++ link
with LLD and remap build prefixes. Helper lookup uses `../lib/osu-radio` before
the existing `/usr/lib/osu-radio` fallback.

`BUILD-INFO.txt` records the build-time commit, dirty status/diff hash, exact
installed Arch packages, package database hashes, tools, flags and binary hashes.
`SOURCE-FILES.sha256` records tracked and untracked source inputs. Staging does
not relabel old binaries with the current checkout's SHA. Dirty builds are
candidate probes; T09 rebuilds the final clean SHA. Reconcile notices with the
actual Rust graph/helper publish **before** staging.

For runtime verification, prepare another root with `base`, `qt6-base`,
`qt6-declarative`, `qt6-svg`, `qt6-wayland`, `alsa-lib`, `dotnet-runtime-10.0`,
`xdg-desktop-portal`, a portal implementation (the check uses
`xdg-desktop-portal-gtk`), and `curl` for the HTTP check. Do not install SDKs or
compilers. Copy only the archive, `runtime-check.sh` and the existing synthetic
`full.realm` fixture as `client.realm` into `/checks`; enter **without** source
or work binds:

```sh
packaging/linux/enter-arch.sh target/arch-runtime/root -- \
  /bin/bash /checks/runtime-check.sh /checks/osu-radio-0.1.0-alpha-arch-x86_64.tar.gz /checks/fixture
```

The check runs from `/`, confirms user SQLite creation and exactly two fixture
beatmaps, verifies the input hash, and starts the GUI's child server. Logs stay
inside the disposable root. SIGTERM cleanup belongs to the check; it does not
prove normal window-close behavior. Run the repository Qt tests/probes, generated
QML lint and scoped Clippy in the build root as well. Offscreen checks do not
prove native X11/Wayland, portal integration or audible playback.

[T05](../../docs/impl/first-release-packaging/T05.md) records actual commands,
versions, hashes and limits; its old Ubuntu report remains historical evidence.
