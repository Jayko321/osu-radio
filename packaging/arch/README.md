# Arch Linux packages

The two x86_64 AUR recipes are mutually exclusive:

- [`osu-radio-git`](osu-radio-git/PKGBUILD) builds the latest `dev` branch from source with system Qt and .NET 10.
- [`osu-radio-bin`](osu-radio-bin/PKGBUILD) installs a versioned Arch archive from a GitHub Release with system Qt and the .NET 10 runtime.

Both provide and conflict with `osu-radio`, install the same executables and preserve XDG data and QSettings on removal. Replacing one variant with the other requires a compatible database schema; arbitrary database downgrades are not supported.

The repository recipes are publication templates. The AUR workflow generates each `.SRCINFO` using `makepkg --printsrcinfo`; for `-bin` it inserts the published release version and archive SHA-256. The checked-in binary checksum is a placeholder, not the published AUR package. See [AUR publication](../aur/README.md) and [release builds](../release/README.md) for the automation and its limitations.

The existing [T07 report](../../docs/impl/first-release-packaging/T07.md) describes historical candidate checks, not acceptance of later builds. Native installation, audio and GUI checks remain distinct from automated builds.
