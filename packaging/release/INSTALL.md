# osu! radio 0.1.0-alpha candidate

This staging directory is for verification. It is not a published release and
does not establish acceptance on Windows or Arch. Check `SHA256SUMS` before use
(`sha256sum -c SHA256SUMS` on Linux; `Get-FileHash -Algorithm SHA256` on Windows).
`manifest.json` records source commit, tools, Qt sources/patches, build parameters,
artifact hashes and the two Rust binaries inside the Arch archive. Its own hash
is in `SHA256SUMS`. These are integrity records, not signatures.

## Windows x86_64

Run `osu-radio-0.1.0-alpha-setup.exe`. Installation defaults to the current user;
select all users to install with administrator rights. Launch osu! radio from
Start. The installer includes Qt, the helper and its .NET runtime; no SDK or
runtime download should be needed. Install a replacement Setup over the same
installation to update. Uninstall through Windows Installed apps. Database,
playlists and QSettings remain in the user profile across these operations.

The acceptance matrix is Windows 10 22H2, Windows 10 LTSC 2021 and Windows 11.
Successful CI on Windows Server 2022 is not acceptance on that matrix.
Qt notices, matching source archives, patches and library replacement instructions
are in `licenses/windows/` and are also supplied to the installer.

## Arch Linux x86_64

Choose one variant; they conflict and provide the same `osu-radio` command:

- `arch/osu-radio-bin`: uses the exact Arch archive in this directory. To check a
  candidate before publication, copy the archive alongside its `PKGBUILD`, then
  run `makepkg --syncdeps --install` as an ordinary user. `makepkg` checks the
  local archive against the final digest without changing the release URL.
  Only declared runtime dependencies are installed; Rust/C++/.NET SDKs are not
  required. Keep a matching portal implementation for the desktop session.
- `arch/osu-radio-git`: run `makepkg --syncdeps --install` as an ordinary user.
  This builds the full current `main` history and requires build dependencies.
  It intentionally follows main, so it is not the fixed-SHA binary candidate.

Build packages in a disposable clean Arch environment before installing them on
a regular profile. Update by installing a newly built package with pacman.
To switch variants, accept pacman's replacement of the conflicting package;
do not delete the user data. Remove with `pacman -R osu-radio-bin` or
`pacman -R osu-radio-git`. Package removal preserves XDG data and QSettings.
An incompatible database must fail without an automatic reset.
The binary archive is a package input, not a portable application for other Linux
distributions. Qt and .NET remain Arch system dependencies.

## Acceptance and final source

Use a separate test profile, explicit stable/lazer directories and read-only
source copies. Check import, audible playback, normal window close and child
shutdown, restart, update and uninstall/reinstall data retention. Record the OS
build and these exact artifact hashes for T10.

The included source archive is generated from the recorded commit. It is not a
downloaded tag archive. Before finalizing, separately authorize and verify the
move of `v0.1.0-alpha`, compare the remote peeled tag and downloaded source archive
with that commit, and repeat CI. The old tag commit was
`bf941c29b2e69fa73aca6c1c6a7c1643f3a7f3f9`; its source archive SHA-256 was
`15fa8e08b960cf00a47ec1ae935bf90c774bfccc17dc82f1100b0bf3668f1566`, and its desktop
entry SHA-256 was `f99337fc9f9d60437461a3e887b0f6259202de5774224f611b85eb6f03b1b2b9`.
The same version will then refer to updated sources. This workflow does not
commit, push, move tags or publish GitHub Release/AUR packages.
