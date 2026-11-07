# osu!lazer Realm parser

This is an implementation-detail helper for `radio-scanner`. It opens osu!lazer's
`client.realm` through Realm .NET in dynamic, read-only mode and writes only NDJSON
to stdout. Diagnostics and failures are written to stderr.

Cargo builds the helper automatically when `radio-scanner` is built. This requires
the .NET 10 SDK. Both the helper and fixture generator target `net10.0` and keep
Realm `20.1.0`. To use an already-built helper instead, set
`OSU_LAZER_REALM_PARSER_PATH` while building or running the Rust application.

At runtime the backend/CLI looks for the helper in this order: a non-empty
`OSU_LAZER_REALM_PARSER_PATH` (an invalid override is an error, never replaced by
another helper); the installed helper relative to the running executable's directory
(`../lib/osu-radio/osu-lazer-realm-parser` on Linux, `helper/osu-lazer-realm-parser.exe`
on Windows); the path baked in at build time.

Build or run it directly with:

```sh
dotnet build tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj
dotnet run --project tools/osu-lazer-realm-parser -- --schema /path/to/client.realm
dotnet run --project tools/osu-lazer-realm-parser -- /path/to/client.realm
```

Publish a directory with an apphost for the target OS (`linux-x64` on Linux,
`win-x64` on Windows):

```sh
dotnet publish tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj -c Release -r linux-x64 --self-contained false -o /absolute/path/helper-fdd
dotnet publish tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj -c Release -r linux-x64 --self-contained true -o /absolute/path/helper-scd
```

The framework-dependent directory needs a .NET 10 runtime; the self-contained
directory includes it. Keep the whole publish directory, including native Realm
libraries. Do not enable trimming, AOT or single-file publishing. See the
[.NET publishing documentation](https://learn.microsoft.com/en-us/dotnet/core/deploying/).
To exercise either directory, set `OSU_LAZER_REALM_PARSER_PATH` to its absolute
apphost path at Cargo build time and run the real-helper tests below. Cargo's
build script tracks changes to that variable. Generate fixtures with the SDK
before testing self-contained execution without a system runtime.

`--schema` emits one JSON object per Realm object type, including its persisted
fields. The default mode emits one `radio-core`-shaped beatmap-set object per
line. Each record uses snake_case field names matching `ImportedBeatmapSet` and
its nested types. Lazer content-addressed file paths are emitted under
`files[].file.resolved_path`; each child beatmap's `metadata.audio_file` remains
the beatmap metadata filename.


Collection import extends normal NDJSON with `type: "collection"` records carrying
`id` (canonical Guid), `name`, and `beatmap_md5_hashes`. Beatmap records keep their
native `hash` and include independent `md5_hash` from Realm `MD5Hash`.
MD5 values are validated and normalized; malformed mandatory collection fields
abort export. A Realm schema without `BeatmapCollection` remains supported.

The [fixture-generator](fixture-generator/Program.cs) uses the existing Realm
package to create genuine synthetic databases. The scanner's
[real-helper tests](../../crates/radio-scanner/src/lazer/tests.rs) build it in
temporary directories and check the production executable and Rust importer:

- Minimal schemas retain compatibility with missing optional metadata/files and
  no collection type; invalid collection hashes/names reject the snapshot.
- A separate rich schema checks metadata/author fields, ordered user tags,
  numeric overflow and content-addressed paths, including null/short hashes.
- `--schema` emits sorted object records with scalar/list/set/dictionary field
  types, nullability, primary keys and index metadata. An unrelated schema can be
  inspected but cannot be imported as beatmaps.
- Invalid arguments, missing/directory inputs and corrupt files use the expected
  exit code and stderr diagnostics. Rich export/schema checks compare Realm bytes
  before/after and assert that media files were not created.

Run `cargo test -p radio-scanner --locked -- real_ --test-threads=1` from the root.
These tests use synthetic Realm data, not a real lazer installation. Native
Windows/macOS behavior and C# source-line coverage remain unverified.
For the .NET 10 Linux publish and no-system-runtime results, see the
[T01 report](../../docs/impl/first-release-packaging/T01.md#отчёт-о-выполнении).
