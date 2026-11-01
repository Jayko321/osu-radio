# osu! radio — Arch Linux system archive (x86_64)

Layout for installation into system directories (`usr/` relative to the install root):

| Path | Content |
| --- | --- |
| `usr/bin/osu-radio-qt`, `usr/bin/osu-radio` (symlink) | Qt desktop app |
| `usr/bin/osu-radio-server` | local server, started by the app from the same directory |
| `usr/lib/osu-radio/` | complete framework-dependent publish of the lazer Realm helper |
| `usr/share/applications/osu-radio.desktop`, `usr/share/icons/hicolor/` | desktop entry and icons |
| `usr/share/licenses/osu-radio/` | project license and all third-party notices (keep every file) |
| `BUILD-INFO.txt`, `SOURCE-FILES.sha256` | build-time source provenance, package/tool versions and input hashes |

Install, for example: `sudo cp -a usr/. /usr/` (or, for a package, copy into the package root).
The app finds the helper relative to its own executable (`../lib/osu-radio`), so no
environment variables, `.env` file or working directory are needed. Data goes to the XDG
directories of the user, never to the install root. Remove by deleting the installed files;
user data is kept.

## Runtime requirements (not included)

This archive targets current Arch x86_64, with the package versions recorded in
`BUILD-INFO.txt`. It does not promise compatibility with other distributions.

- `glibc`, `libgcc`, `libstdc++`, `alsa-lib` (see actual ELF symbol requirements in `BUILD-INFO.txt`).
- `qt6-base`, `qt6-declarative`, `qt6-svg`, `qt6-wayland`, at least the Qt version
  used to build this archive; these supply libraries, QML modules and platform plugins.
- `dotnet-runtime-10.0` for the helper, including the system .NET host; no SDK.
- `xdg-desktop-portal` and a desktop-appropriate portal implementation for folder pickers.

Qt and the .NET shared runtime are system dependencies, not bundled files.
No SDK, compiler or source tree is needed to run it.
