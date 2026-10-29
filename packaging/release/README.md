# Release candidate staging

[`release-staging.yml`](../../.github/workflows/release-staging.yml) is manual and
has only `contents: read`. Supply a full lowercase source commit in `source_sha`.
All jobs check out that commit; the Windows and Arch jobs call the existing
T08/T05 builders, and the final Arch job calls T07 recipe generation as a
non-root user. No workflow step commits, moves a tag or publishes a release.

**Not ready to run:** the selected commit must contain these files, and
[`inputs.json`](inputs.json) still lacks the reviewed Windows materials URL and
SHA-256. The workflow stops at preflight while either is absent. The dirty local
checkout and the older T08 branch are not a substitute for this source commit.
Completing T08's license/source inventory and selecting/committing the desired
source remain necessary. Do not fill these fields with diagnostic T08 notices.

## Build inputs

The Arch image is pinned by its official Docker registry manifest digest. Arch
packages come from the dated archive snapshot, with package signature checks
enabled. The full installed package list and package database hashes are captured
by T05. On 2026-10-29, the image manifest digest was checked against its bytes;
the `core.db` snapshot returned HTTP 200 with SHA-256
`0b720f1750ea22fd3d7b571a09f41832c9805f0f62838eb0e5f389967785236d`.
Neither the image nor the workflow has been run in this task.

Windows Qt/.NET/MSVC/Windows SDK versions and the Inno installer hash are from
the actual T08 candidate, not a newly validated release environment. T08's builder
checks exact tool versions; the wrapper checks the downloaded Inno installer hash
and signature before deriving the ISCC hash. Qt uses the pinned implementation
action directly, avoiding its composite wrapper's mutable nested action tags.
Python is 3.13.3, aqtinstall 3.3.0 and py7zr 1.0.0. Rust remains selected by
`rust-toolchain.toml`. Hosted runner images can drift; unavailable tool versions
must fail instead of silently changing the pins. Runtime file hashes are in the
Windows payload inventory. The action/installer pins do not constitute a license
audit or proof that downloaded Qt corresponds to the supplied source materials.

Arch builds use `/work` inside the disposable container, matching T05's helper
build path. Its current exact helper inventory remains enforced. Any changed
helper output, runtime or dependency requires reconciliation of that inventory
before staging; the workflow does not rewrite it to make a mismatch pass.

## Windows materials handoff

`windows_materials_url` must identify an HTTPS `.tar.gz`; its SHA-256 is pinned
in the same source commit. Extraction uses Python's data filter and rejects
links/device entries. The archive root contains:

- `QT-NOTICES.txt`, `DOTNET-NOTICES.txt`, `REALM-NOTICES.txt`, `MSVC-NOTICES.txt`,
  `RUST-NOTICES.txt`, `ASSET-NOTICES.txt`: complete applicable texts for the actual
  Windows payload, including the self-contained .NET and native DLLs. Additional
  component inventories and full texts can accompany these files.
- `LICENSE`: the project license, identical to the selected source checkout.
- `REPLACING-QT.md`: instructions applicable to the supplied dynamic Qt build.
- `QT-SOURCES.json`: `qt_version`, nonempty `sources` and explicit `patches`
  arrays. Each source/patch has `file` (relative path in the materials directory),
  `url` (HTTPS origin) and `sha256`. Include the matching source archives and
  patches themselves. An empty `patches` array explicitly means unmodified
  upstream. Review that statement against the actual Qt binary distribution.

These filenames are the T09 handoff contract; they do not claim T08 has already
produced or reviewed those materials. Mechanical checks establish presence and
integrity only. A license audit must establish completeness and correspondence
to Qt/.NET/Realm/MSVC/Rust/assets before setting the URL/hash. The old Arch
inventory, arbitrary nonempty files or a renamed README cannot establish that.
T08's `NOT-RELEASE-READY.txt` is rejected. All supplied materials go into both
Setup and the candidate staging directory through the existing T08 builder.

## Assembly and checks

The Windows handoff contains `output/osu-radio-0.1.0-alpha-setup.exe`,
`setup.sha256`, `BUILD-INFO.txt`, `payload.sha256` and `licenses/`. The Arch
handoff is `osu-radio-0.1.0-alpha-arch-x86_64.tar.gz`. Both must record the same
clean source SHA. Windows Setup and transferred metadata/materials are checked
against their build-time hashes; Arch Rust binary hashes are computed from
archive members. Arch notices must equal those in the selected source. Recorded
provenance cannot independently prove which source produced machine code.

In an Arch environment with `makepkg`, as an ordinary user:

```sh
python3 -B packaging/release/stage.py check FULL_SOURCE_SHA
python3 -B packaging/release/stage.py assemble FULL_SOURCE_SHA \
  /path/osu-radio-0.1.0-alpha-arch-x86_64.tar.gz /path/windows-handoff /path/new-candidate
cd /path/new-candidate
sha256sum --check SHA256SUMS
```

The output includes Setup, the Arch archive, both recipes and generated
`.SRCINFO`, licenses/Qt sources, a commit source archive, installation
instructions, build metadata, `manifest.json` and `SHA256SUMS`. Hashes are made
after file generation. The manifest lists every other final file; `SHA256SUMS`
also covers the manifest. Neither file hashes itself. A temporary sibling
directory is renamed only after all checks and generation succeed, so failure
does not leave a successful-looking partial candidate. CI uploads this isolated
directory with hidden files enabled to preserve `.SRCINFO`; no checkout/caches
are included. Build logs are separate artifacts retained even after failures.

Run the focused synthetic handoff/self-check and existing T08 checks:

```sh
python3 -B -m unittest discover -s packaging/release -p 'test_*.py' -v
python3 -B -m unittest discover -s packaging/windows -p 'test_*.py' -v
```

The tests use real Git and `makepkg`, fake binaries and license texts. They check
integrity and rejection behavior, not actual Setup contents, license sufficiency,
the Windows GUI or audio. [T09](../../docs/impl/first-release-packaging/T09.md)
records actual evidence and outstanding final tag/CI acceptance. User-facing
candidate installation and finalization instructions are in [INSTALL.md](INSTALL.md).

Upstream references: [workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax),
[artifact uploads and hidden files](https://github.com/actions/upload-artifact),
[artifact downloads](https://github.com/actions/download-artifact),
[official Arch images](https://github.com/archlinux/archlinux-docker).
