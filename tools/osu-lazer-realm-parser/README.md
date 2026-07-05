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
fields. The default mode emits one `radio-core`-shaped beatmap object per line.
Each record uses snake_case field names matching `ImportedBeatmap` and nested
types. Lazer content-addressed file paths are emitted under
`beatmap_set.files[].file.resolved_path`; `metadata.audio_file` remains the
beatmap metadata filename.
