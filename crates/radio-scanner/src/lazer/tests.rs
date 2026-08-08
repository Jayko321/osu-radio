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
        Some(Path::new(r#"C:\osu\files\a\ab\abc"#))
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
