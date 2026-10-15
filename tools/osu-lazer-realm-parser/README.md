# osu!lazer Realm parser

This is an implementation-detail helper for `radio-scanner`. It opens osu!lazer's
`client.realm` through Realm .NET in dynamic, read-only mode and writes only NDJSON
to stdout. Diagnostics and failures are written to stderr.

Cargo builds the helper automatically when `radio-scanner` is built. This requires
the .NET 8 SDK. To use an already-built helper instead, set
`OSU_LAZER_REALM_PARSER_PATH` while building or running the Rust application.

Build or run it directly with:

```sh
dotnet build tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj
dotnet run --project tools/osu-lazer-realm-parser -- --schema /path/to/client.realm
dotnet run --project tools/osu-lazer-realm-parser -- /path/to/client.realm
```

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
