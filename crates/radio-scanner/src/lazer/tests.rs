use std::path::Path;

use radio_core::OsuKind;

use super::types::parse_lazer_beatmap_set_line;

#[test]
fn parses_lazer_beatmap_set_json_into_core_type() {
    let beatmap_set = parse_lazer_beatmap_set_line(
        r#"{"source":"Lazer","online_id":123,"hash":"set-hash","files":[{"filename":"audio.mp3","file":{"hash":"abc","resolved_path":"C:\\osu\\files\\a\\ab\\abc"}}],"beatmaps":[{"difficulty_name":"Hard","bpm":180.5,"hash":"beatmap-hash","metadata":{"title":"Song","title_unicode":"Song Unicode","artist":"Artist","artist_unicode":"Artist Unicode","author":{"online_id":456,"username":"Mapper","country_code":"EE"},"source":"Game","tags":"tag one","user_tags":["favorite"],"preview_time":12345,"audio_file":"audio.mp3","background_file":"bg.jpg"}}]}"#,
    )
    .unwrap();

    assert_eq!(beatmap_set.source, OsuKind::Lazer);
    assert_eq!(beatmap_set.online_id, Some(123));
    assert_eq!(beatmap_set.hash.as_deref(), Some("set-hash"));
    assert_eq!(beatmap_set.files.len(), 1);
    assert_eq!(beatmap_set.files[0].filename.as_deref(), Some("audio.mp3"));
    assert_eq!(
        beatmap_set.files[0]
            .file
            .as_ref()
            .and_then(|file| file.hash.as_deref()),
        Some("abc")
    );
    assert_eq!(
        beatmap_set.files[0]
            .file
            .as_ref()
            .and_then(|file| file.resolved_path.as_deref()),
        Some(Path::new(r"C:\osu\files\a\ab\abc"))
    );

    assert_eq!(beatmap_set.beatmaps.len(), 1);
    let beatmap = &beatmap_set.beatmaps[0];
    assert_eq!(beatmap.difficulty_name.as_deref(), Some("Hard"));
    assert_eq!(beatmap.bpm, Some(180.5));
    assert_eq!(beatmap.hash.as_deref(), Some("beatmap-hash"));

    let metadata = beatmap.metadata.as_ref().unwrap();
    assert_eq!(metadata.title.as_deref(), Some("Song"));
    assert_eq!(metadata.title_unicode.as_deref(), Some("Song Unicode"));
    assert_eq!(metadata.artist.as_deref(), Some("Artist"));
    assert_eq!(metadata.artist_unicode.as_deref(), Some("Artist Unicode"));
    let author = metadata.author.as_ref().unwrap();
    assert_eq!(author.online_id, Some(456));
    assert_eq!(author.username.as_deref(), Some("Mapper"));
    assert_eq!(author.country_code.as_deref(), Some("EE"));
    assert_eq!(metadata.source.as_deref(), Some("Game"));
    assert_eq!(metadata.tags.as_deref(), Some("tag one"));
    assert_eq!(metadata.user_tags, vec!["favorite"]);
    assert_eq!(metadata.preview_time, Some(12345));
    assert_eq!(metadata.audio_file.as_deref(), Some("audio.mp3"));
    assert_eq!(metadata.background_file.as_deref(), Some("bg.jpg"));
}

#[test]
fn resolves_audio_and_background_by_exact_named_file_usage() {
    let set = parse_lazer_beatmap_set_line(r#"{"source":"Lazer","files":[{"filename":"audio.mp3","file":{"resolved_path":"/files/audio-hash"}},{"filename":"bg.jpg","file":{"resolved_path":"/files/background-hash"}},{"filename":"bg.jpg","file":{"resolved_path":"/files/later-match"}}],"beatmaps":[{"metadata":{"audio_file":"audio.mp3","background_file":"bg.jpg"}},{"metadata":{"background_file":"BG.jpg"}},{"metadata":null}]}"#).unwrap();
    assert_eq!(
        set.resolved_audio_path(&set.beatmaps[0]),
        Some(Path::new("/files/audio-hash"))
    );
    assert_eq!(
        set.resolved_background_path(&set.beatmaps[0]),
        Some(Path::new("/files/background-hash"))
    );
    assert_eq!(set.resolved_background_path(&set.beatmaps[1]), None);
    assert_eq!(set.resolved_background_path(&set.beatmaps[2]), None);
}

#[test]
fn rejects_beatmap_first_json() {
    let error = parse_lazer_beatmap_set_line(
        r#"{"source":"Lazer","difficulty_name":"Hard","beatmap_set":{"online_id":123}}"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("beatmap set JSON"));
}

#[cfg(unix)]
mod helper_process {
    use std::{
        env, fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::super::scanner::import_from_lazer_realm_with_helper;

    #[tokio::test]
    async fn accepts_a_fake_ndjson_helper() {
        let helper = fake_helper(
            "printf '%s\\n' '{\"source\":\"Lazer\",\"beatmaps\":[{\"metadata\":{\"title\":\"test\"}}]}'",
        );
        let result = import_from_lazer_realm_with_helper(Path::new("unused.realm"), &helper).await;
        fs::remove_file(helper).unwrap();

        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn includes_helper_stderr_in_failure() {
        let helper = fake_helper("echo 'realm exploded' >&2\nexit 7");
        let error = import_from_lazer_realm_with_helper(Path::new("unused.realm"), &helper)
            .await
            .unwrap_err();
        fs::remove_file(helper).unwrap();

        assert!(error.to_string().contains("realm exploded"));
    }

    fn fake_helper(body: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "osu-lazer-realm-parser-test-{}-{nonce}",
            std::process::id()
        ));
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();

        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }
}

#[test]
fn validates_collection_records_and_normalizes_md5_without_changing_native_hashes() {
    use super::types::{LazerRecord, parse_lazer_line};
    let LazerRecord::Collection(collection) = parse_lazer_line(r#"{"type":"collection","id":"A1234567-89AB-CDEF-0123-456789ABCDEF","name":"","beatmap_md5_hashes":["ABCDEF0123456789ABCDEF0123456789"]}"#).unwrap() else { panic!("expected collection") };
    assert_eq!(collection.source_id, "a1234567-89ab-cdef-0123-456789abcdef");
    assert_eq!(collection.name, "");
    assert_eq!(
        collection.beatmap_md5_hashes,
        vec!["abcdef0123456789abcdef0123456789"]
    );
    let set = parse_lazer_beatmap_set_line(r#"{"source":"Lazer","hash":"Native-HASH","beatmaps":[{"hash":"Native-HASH","md5_hash":"ABCDEF0123456789ABCDEF0123456789"}]}"#).unwrap();
    assert_eq!(set.hash.as_deref(), Some("Native-HASH"));
    assert_eq!(set.beatmaps[0].hash.as_deref(), Some("Native-HASH"));
    assert_eq!(
        set.beatmaps[0].md5_hash.as_deref(),
        Some("abcdef0123456789abcdef0123456789")
    );
    for line in [
        r#"{"type":"collection","id":"bad","name":"test","beatmap_md5_hashes":[]}"#,
        r#"{"type":"collection","id":"a1234567-89ab-cdef-0123-456789abcdef","beatmap_md5_hashes":[]}"#,
        r#"{"type":"collection","id":"a1234567-89ab-cdef-0123-456789abcdef","name":"test"}"#,
        r#"{"type":"collection","id":"a1234567-89ab-cdef-0123-456789abcdef","name":"test","beatmap_md5_hashes":["bad"]}"#,
        r#"{"type":"schema"}"#,
        r#"{"source":"Lazer","beatmaps":[{"md5_hash":"bad"}]}"#,
    ] {
        assert!(parse_lazer_line(line).is_err(), "{line}");
    }
}

#[tokio::test]
async fn imports_real_realm_fixtures_with_the_production_helper() {
    use super::scanner::import_snapshot_from_lazer_realm_with_helper;
    use std::{path::PathBuf, process::Command};
    let temp = tempfile::TempDir::new().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = root.join("tools/osu-lazer-realm-parser/fixture-generator");
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    for filename in ["fixture-generator.csproj", "Program.cs"] {
        std::fs::copy(source.join(filename), project_dir.join(filename)).unwrap();
    }
    let project = project_dir.join("fixture-generator.csproj");
    let output = temp.path().join("generator");
    let dotnet_home = std::env::var_os("DOTNET_CLI_HOME")
        .map_or_else(|| root.join("target/dotnet-home"), PathBuf::from);
    let build = Command::new("dotnet")
        .args([
            "build",
            "--configuration",
            "Release",
            "--nologo",
            "--output",
        ])
        .arg(&output)
        .arg(&project)
        .arg("--property:NuGetAudit=false")
        .env("DOTNET_CLI_HOME", dotnet_home)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "fixture generator failed: {} {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let fixtures = temp.path().join("fixtures");
    let generate = Command::new("dotnet")
        .arg(output.join("fixture-generator.dll"))
        .arg(&fixtures)
        .output()
        .unwrap();
    assert!(
        generate.status.success(),
        "fixture generation failed: {}",
        String::from_utf8_lossy(&generate.stderr)
    );
    let helper = Path::new(env!("OSU_LAZER_REALM_PARSER_BUILT_PATH"));
    let direct = Command::new(helper)
        .arg(fixtures.join("full.realm"))
        .output()
        .unwrap();
    assert!(
        direct.status.success(),
        "production helper failed: {}",
        String::from_utf8_lossy(&direct.stderr)
    );
    let protocol = String::from_utf8(direct.stdout).unwrap();
    assert!(protocol.contains("\"type\":\"collection\""));
    let snapshot =
        import_snapshot_from_lazer_realm_with_helper(&fixtures.join("full.realm"), helper)
            .await
            .unwrap();
    assert_eq!(snapshot.beatmap_sets.len(), 1);
    assert_eq!(snapshot.beatmap_sets[0].beatmaps.len(), 2);
    assert_eq!(
        snapshot.beatmap_sets[0].beatmaps[0].hash.as_deref(),
        Some("Native-Hash-Is-NOT-MD5")
    );
    assert_eq!(
        snapshot.beatmap_sets[0].beatmaps[0].md5_hash.as_deref(),
        Some("abcdef0123456789abcdef0123456789")
    );
    assert_eq!(snapshot.collections.len(), 2);
    assert_eq!(snapshot.collections[0].name, "好きな曲 🎵");
    assert_eq!(
        snapshot.collections[0].source_id,
        "a1234567-89ab-cdef-0123-456789abcdef"
    );
    assert_eq!(snapshot.collections[0].beatmap_md5_hashes.len(), 3);
    assert_eq!(
        snapshot.collections[0].beatmap_md5_hashes[0],
        snapshot.collections[0].beatmap_md5_hashes[2]
    );
    assert!(snapshot.collections[1].beatmap_md5_hashes.is_empty());
    let without = import_snapshot_from_lazer_realm_with_helper(
        &fixtures.join("no-collections.realm"),
        helper,
    )
    .await
    .unwrap();
    assert!(without.collections.is_empty());
    for filename in ["invalid-md5.realm", "missing-name.realm"] {
        assert!(
            import_snapshot_from_lazer_realm_with_helper(&fixtures.join(filename), helper)
                .await
                .is_err(),
            "{filename}"
        );
    }
}
