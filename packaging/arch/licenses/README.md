# osu! radio distribution notices

Install the root project `LICENSE` as `/usr/share/licenses/osu-radio/LICENSE`
and **every file in this directory** alongside it. Preserve the filenames and
complete contents. T04 owns installation; T05 checks the resulting package.
These are prepared materials, not evidence of an installed release.

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
or additional rights to the osu! branding. The original Figma source and node
identifiers are retained in the asset provenance file:
[Figma folder reference](https://www.figma.com/design/VQULSEzRKI4ki7uWGJEezk/osu-radio--Copy-?node-id=415-244),
stable node `415:250`, lazer node `415:279` (downloaded 2026-09-28).

## Rust and native components

[RUST-PACKAGES.tsv](RUST-PACKAGES.tsv) records all 308 registry packages in the
selected Linux normal/build dependency graph: exact versions, declared licenses,
graph roles, enabled features, Cargo.lock source checksums, source archive URLs
and references to the full texts in [RUST-NOTICES.txt](RUST-NOTICES.txt).
Identical full texts are stored once, under their SHA-256. Copyright lines from
source and compound-license notices are also retained, including ring's
BoringSSL/fiat notices, encoding_rs's WHATWG notice, Unicode and matchit notices.
Build tools and proc macros are included conservatively; an inventory entry is
not a claim that its standalone binary is installed.

Inventory basis: commit `48d2c0997ea7d6125d4bf421d692e499c0c3ab0d`, unchanged
Cargo.lock, `x86_64-unknown-linux-gnu`, Qt and server default features (SQLite and
API docs; no PostgreSQL). Commands executed on 2026-10-17:

```sh
cargo metadata --offline --locked --format-version 1 --filter-platform x86_64-unknown-linux-gnu
cargo tree -p osu-radio-qt -p osu-radio-server --locked --offline --target x86_64-unknown-linux-gnu --edges normal,build --prefix none --format '{p}|{f}'
```

This is a resolved build graph, not a symbol-level assertion about optimized
executables. T04/T05 must compare it with the final build flags and lockfile.
Changing features, versions, target or toolchain requires another comparison.

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
output for `net8.0/linux-x64`, with file hashes and origins.
[HELPER-NOTICES.txt](HELPER-NOTICES.txt) contains complete licenses and notices for:

- Realm 20.1.0 (`Realm.dll`, native wrapper), Realm Core 20.1.2 and its bundled
  Intel decimal math, Bison parser with skeleton exception, JSON, MPark and S2
  components; additional jsonsl terms are retained conservatively.
- MongoDB.Bson 2.21.0 and Remotion.Linq 2.2.0, the other two published NuGet DLLs.
- Microsoft.NETCore.App.Host.linux-x64 8.0.32 (the generated executable host).
- GCC 11.2.0 C++ runtime code statically present in the prebuilt Realm library,
  including the complete GPLv3 text and GCC Runtime Library Exception 3.1.

Realm also statically includes OpenSSL 3.3.1. Its full Apache license and original
source-comment notices, including additional terms, are in
[OPENSSL-NOTICES.txt](OPENSSL-NOTICES.txt). Its version is confirmed by Realm's
`dependencies.yml` and the actual native library; `ldd` alone does not reveal it.

The copied helper project was published outside the source tree with:

```sh
dotnet publish osu-lazer-realm-parser.csproj -c Release -r linux-x64 --self-contained false -o publish --nologo
```

Final verification used the official SDK 8.0.426 in a temporary directory; its
archive was verified against the SHA-512 in Microsoft's release metadata.
The unchanged project files included `FodyWeavers.xml`. No system SDK or project
configuration was changed. T04/T05 must repeat the inventory against the final
package build and its apphost pack version.
Fody is build-time only. The other restored Microsoft/System assemblies are
provided by the system .NET shared framework and were absent from this publish;
listing the entire restore graph as shipped DLLs would be incorrect.

Qt, ALSA, glibc and the dynamically linked Rust/Qt executable dependencies belong
to their system packages, as does the .NET shared runtime. This package does not
redistribute those libraries. Realm's native library itself resolves only glibc
libraries (`libdl`, `libm`, `libpthread`, `libc` and the loader); its static
OpenSSL and C++ code are consequently covered here rather than assigned to
system packages. Final package dependency/link analysis belongs to T05.

## Release handoff

T04 must install the root MIT and all these materials under the agreed license
directory. T05 must recheck Cargo.lock/features, Rust toolchain, helper publish,
apphost/native payload and the installed notices. No package, clean chroot,
KDE launch, game import or audio acceptance is claimed by this directory.
