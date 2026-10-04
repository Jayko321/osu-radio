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

The `fixture-generator` project uses the existing Realm package to create genuine
synthetic databases. The scanner's `imports_real_realm_fixtures_with_the_production_helper`
test builds this project and creates all fixture databases in temporary directories,
then checks both the production executable and Rust importer.
