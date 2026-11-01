# osu! radio distribution notices

Install the root project `LICENSE` as `/usr/share/licenses/osu-radio/LICENSE`
and **every file in this directory** alongside it. Preserve the filenames and
complete contents. The Linux system archive built by
[packaging/linux](../../linux/README.md) already carries them; Arch recipes reuse that layout. These are prepared materials, not evidence of an
installed release. `HELPER-FILES.tsv` records a historical helper build;
`BUILD-INFO.txt` in each archive lists the files and hashes from that build.

The root MIT license covers project code. Its copyright name and year come from
the repository's initial author records (2026, Василий Коршунов); the workspace
already declares MIT. Bundled third parties retain their respective terms.

## Embedded assets

Qt's `build.rs` recursively embeds the entire `apps/osu-radio-qt/assets/` tree.
The following notices are byte-for-byte copies of its existing full texts:

| Components | Installed notice | Source in the repository |
| --- | --- | --- |
| Nunito variable font | [NUNITO-OFL.txt](NUNITO-OFL.txt) | `apps/osu-radio-qt/assets/fonts/OFL.txt` |
| Poppins Regular, Medium, SemiBold, Bold | [POPPINS-OFL.txt](POPPINS-OFL.txt) | `apps/osu-radio-qt/assets/fonts/Poppins-OFL.txt` |
| Lucide icons and Feather-derived shapes, including the three 16px design variants | [LUCIDE-LICENSE](LUCIDE-LICENSE) | `apps/osu-radio-qt/assets/icons/LUCIDE-LICENSE` |
| RemixIcon v4.2.0: pencil-line, search-line, volume-up-fill, settings-4-line, arrow-down-s-line | [REMIX-LICENSE](REMIX-LICENSE) | `apps/osu-radio-qt/assets/icons/REMIX-LICENSE` |
| App-authored pause.svg | Project MIT `LICENSE` | Root project license |
| logos/stable.png and logos/lazer.png | Maintainer's permission statement below | [Asset provenance](https://github.com/Jayko321/osu-radio/blob/48d2c0997ea7d6125d4bf421d692e499c0c3ab0d/apps/osu-radio-qt/assets/SOURCES.md) |

The maintainer confirmed on 2026-10-17 that they have oral permission to
redistribute both logo images. No separate license text or permission document
was supplied. This records that confirmation without granting an MIT sublicense
or additional rights to the osu! branding. The package icons in
`packaging/icons/` are downscaled derivatives of `lazer.png` with the same status. The original Figma source and node
identifiers are retained in the asset provenance file:
[Figma folder reference](https://www.figma.com/design/VQULSEzRKI4ki7uWGJEezk/osu-radio--Copy-?node-id=415-244),
stable node `415:250`, lazer node `415:279` (downloaded 2026-09-28).

## Rust and native components

[RUST-PACKAGES.tsv](RUST-PACKAGES.tsv) records all 366 registry packages in the
selected Linux normal/build dependency graph: exact versions, declared licenses,
graph roles, enabled features, Cargo.lock source checksums, source archive URLs
and references to the full texts in [RUST-NOTICES.txt](RUST-NOTICES.txt).
Identical full texts are stored once, under their SHA-256. Copyright lines from
source and compound-license notices are also retained, including ring's
BoringSSL/fiat notices, encoding_rs's WHATWG notice, Unicode and matchit notices.
Build tools and proc macros are included conservatively; an inventory entry is
not a claim that its standalone binary is installed.

Inventory basis: working checkout based on
`c9f2500bad0c645d4e5e2db5dcdd9f6b712ac551` (uncommitted changes present), Cargo.lock
SHA-256 `a9038e8690b970f54515a8e3a6b101214b100366ef0bc902e72539be39ff6ed0`,
`x86_64-unknown-linux-gnu`, Qt and server default features (SQLite and
API docs; no PostgreSQL). Graph, features, lock checksums, 248 full-text hashes and embedded asset notices
rechecked on 2026-10-29 in the isolated Arch build:

```sh
cargo tree -p osu-radio-qt -p osu-radio-server --locked --offline --target x86_64-unknown-linux-gnu --edges normal,build --prefix none --format '{p}|{f}'
```

Compared with the 2026-10-20 inventory, the graph has 366 instead of 363 packages;
the new ones are `arc-swap` 1.9.3 and `ksni` 0.3.7 (system tray) and the proc macro
`pastey` 0.2.3. Their checksums equal Cargo.lock and their texts come from the
exact crate archives. No other package, version or enabled feature changed.
This is a resolved build graph, not a symbol-level assertion about optimized
executables. It describes the current checkout, including Signalsmith, MPRIS and the
tray, not the older published `v0.1.0-alpha` archive still pinned in PKGBUILD.
Before a new release, compare it with the final build flags and lockfile.
Changing features, versions, target or toolchain requires another comparison.

The 2026-10-20 comparison had added 55 previously absent packages (DSP/bindgen,
MPRIS/zbus and their transitive normal/build dependencies), and refreshed graph
roles and enabled features for existing entries. Build roles include transitive
build dependencies and proc macros. `RUST-NOTICES.txt` retains the complete
MIT texts from the published `signalsmith-stretch 0.1.3` crate for:

- Rust wrapper: `LICENSE.md`, Copyright 2024 Colin Marc.
- Embedded Signalsmith Stretch C++: `signalsmith-stretch/LICENSE.txt`,
  Copyright 2022 Geraint Luff / Signalsmith Audio Ltd.
- Embedded Signalsmith Linear C++: `signalsmith-linear/LICENSE.txt`,
  Copyright 2025 Signalsmith Audio.

These embedded sources are covered by that exact crate archive/checksum;
they are not independently resolved Cargo packages. libclang/bindgen run only
during the build; the compiled DSP is bundled, not a new runtime shared library.
The dasp crates omit license files from their published archives: their full MIT
and Apache copyright notices were retrieved at each `.cargo_vcs_info.json` SHA.
The complete Apache-2.0 terms are also retained. mpris-server's unmodified
MPL-2.0 source is available from its exact inventory source URL, with the same
source/modification/redistribution rights described below for Symphonia.

The full upstream texts absent from published crate archives were obtained from
the commits in their `.cargo_vcs_info.json`; each text's source is recorded.
Two build-time proc macros need explicit provenance qualifications: sea-bae
0.2.2 declares MIT but supplies no standalone notice in either the fork or its
bae parent; its metadata author and the full unfilled MIT terms are retained.
sea-schema-derive 0.3.0 supplies an unresolvable VCS SHA; its exact source archive
is checksummed and its shared repository MIT/Apache texts are identified
separately, without claiming that the license-source tag is its source commit.

Symphonia's unmodified MPL-2.0 source is available from each exact crate source
URL in the inventory and notices. Those notices explicitly preserve the
recipient's rights to obtain, modify and redistribute that source under MPL.

Rust 1.98.0 statically contributes its standard library. The toolchain's complete
library copyright document is [RUST-STDLIB-COPYRIGHT.html](RUST-STDLIB-COPYRIGHT.html);
its general license texts and source location are in
[RUST-STDLIB-LICENSES.txt](RUST-STDLIB-LICENSES.txt). These are the library notices,
not a list of the entire compiler's dependencies.

The bundled SQLite 3.51.3 copyright disclaimer is in
[SQLITE-NOTICE.txt](SQLITE-NOTICE.txt); libsqlite3-sys's separate wrapper MIT text
is in the Rust notices. ring's embedded C/assembly code is covered there too.

## Framework-dependent lazer helper

[HELPER-FILES.tsv](HELPER-FILES.tsv) records the actual isolated Release publish
output for `net10.0/linux-x64` (framework-dependent), with file sizes, hashes and
origins, taken on 2026-10-29 from the Arch build in
[packaging/linux](../../linux/README.md). The `osu-lazer-realm-parser.dll`, `.pdb`
and `.deps.json` hashes belong to that build and will differ in another one.
[HELPER-NOTICES.txt](HELPER-NOTICES.txt) contains complete licenses and notices for:

- Realm 20.1.0 (`Realm.dll`, native wrapper), Realm Core 20.1.2 and its bundled
  Intel decimal math, Bison parser with skeleton exception, JSON, MPark and S2
  components; additional jsonsl terms are retained conservatively. The Realm package
  has no `net10.0` asset; its `lib/net8.0/Realm.dll` is the byte-identical file
  published before and loads on the .NET 10 runtime.
- MongoDB.Bson 2.21.0 and Remotion.Linq 2.2.0, the other two published NuGet DLLs.
- Microsoft.NETCore.App.Host.linux-x64 10.0.13 (the generated executable host; its
  LICENSE and THIRD-PARTY-NOTICES were taken from that exact NuGet package, replacing
  the earlier candidate texts).
- GCC 11.2.0 C++ runtime code statically present in the prebuilt Realm library,
  including the complete GPLv3 text and GCC Runtime Library Exception 3.1.

Realm also statically includes OpenSSL 3.3.1. Its full Apache license and original
source-comment notices, including additional terms, are in
[OPENSSL-NOTICES.txt](OPENSSL-NOTICES.txt). Its version is confirmed by Realm's
`dependencies.yml` and the actual native library; `ldd` alone does not reveal it.

The publish uses the Arch .NET SDK 10.0.113 (apphost pack 10.0.13) with:

```sh
dotnet publish osu-lazer-realm-parser.csproj --configuration Release --runtime linux-x64 --self-contained false --output publish
```

from a copy of the project's three source files (`.csproj`, `Program.cs`,
`FodyWeavers.xml`). The system provides the .NET 10 runtime (`Microsoft.NETCore.App`
10.0.x); this archive does not redistribute it. The self-contained Windows Setup bundles that runtime and needs its own inventory (T08).
Fody is build-time only. The other restored Microsoft/System assemblies are
provided by the system .NET shared framework and were absent from this publish;
listing the entire restore graph as shipped DLLs would be incorrect.

Qt, ALSA, glibc and the dynamically linked Rust/Qt executable dependencies belong
to their system packages, as does the .NET shared runtime. This package does not
redistribute those libraries. Realm's native library itself resolves only glibc
libraries (`libdl`, `libm`, `libpthread`, `libc` and the loader); its static
OpenSSL and C++ code are consequently covered here rather than assigned to
system packages.

The current Arch archive records link analysis in `BUILD-INFO.txt`. Qt plugins
and QML modules load from system packages. The old Ubuntu candidate's GLIBC_2.34
requirement does not establish the requirement of this Arch build. AppImage is
deferred; no Qt or .NET shared runtime is bundled by this archive. Windows Qt
and runtime notices must be checked separately in T08.

## Release handoff

The materials above describe the 2026-10-29 candidate probe of the Arch system archive
(working checkout based on `c9f2500`, not a release). The final release must recheck
Cargo.lock/features, Rust toolchain, helper publish, apphost/native payload and the
installed notices against the clean commit (T09), and the Windows and self-contained
helper payloads need their own inventories (T08). No package, KDE launch, game import
or audio acceptance is claimed by this directory.
