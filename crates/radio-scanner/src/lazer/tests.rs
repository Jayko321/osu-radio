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

#[test]
fn missing_optional_wire_fields_remain_absent_and_only_documented_lists_default() {
    let set = parse_lazer_beatmap_set_line(
        r#"{"source":"Lazer","beatmaps":[{},{"metadata":{}},{"metadata":{"author":{}}}]}"#,
    )
    .unwrap();
    assert!(set.files.is_empty());
    assert_eq!(set.online_id, None);
    assert_eq!(set.hash, None);
    let missing = &set.beatmaps[0];
    assert_eq!(missing.difficulty_name, None);
    assert_eq!(missing.bpm, None);
    assert_eq!(missing.hash, None);
    assert_eq!(missing.md5_hash, None);
    assert!(missing.metadata.is_none());
    let metadata = set.beatmaps[1].metadata.as_ref().unwrap();
    assert_eq!(metadata.title, None);
    assert_eq!(metadata.title_unicode, None);
    assert_eq!(metadata.artist, None);
    assert_eq!(metadata.artist_unicode, None);
    assert!(metadata.author.is_none());
    assert_eq!(metadata.source, None);
    assert_eq!(metadata.tags, None);
    assert!(metadata.user_tags.is_empty());
    assert_eq!(metadata.preview_time, None);
    assert_eq!(metadata.audio_file, None);
    assert_eq!(metadata.background_file, None);
    let author = set.beatmaps[2]
        .metadata
        .as_ref()
        .unwrap()
        .author
        .as_ref()
        .unwrap();
    assert_eq!(author.online_id, None);
    assert_eq!(author.username, None);
    assert_eq!(author.country_code, None);
}

#[test]
fn wire_types_reject_missing_required_fields_null_lists_and_invalid_nested_values() {
    for line in [
        "{}",
        r#"{"source":"Lazer"}"#,
        r#"{"beatmaps":[]}"#,
        r#"{"source":"Stable","beatmaps":[]}"#,
        r#"{"source":"lazer","beatmaps":[]}"#,
        r#"{"source":"Lazer","beatmaps":null}"#,
        r#"{"source":"Lazer","files":null,"beatmaps":[]}"#,
        r#"{"source":"Lazer","online_id":2147483648,"beatmaps":[]}"#,
        r#"{"source":"Lazer","files":[{"file":{"resolved_path":42}}],"beatmaps":[]}"#,
        r#"{"source":"Lazer","beatmaps":[{"bpm":"fast"}]}"#,
        r#"{"source":"Lazer","beatmaps":[{"metadata":{"user_tags":null}}]}"#,
        r#"{"source":"Lazer","beatmaps":[{"metadata":{"tags":["tag"]}}]}"#,
        r#"{"source":"Lazer","beatmaps":[{"metadata":{"preview_time":1.5}}]}"#,
        r#"{"source":"Lazer","beatmaps":[{"metadata":{"author":{"online_id":2147483648}}}]}"#,
    ] {
        assert!(
            parse_lazer_beatmap_set_line(line).is_err(),
            "accepted {line}"
        );
    }
}

#[test]
fn collection_guid_validates_every_character_and_record_types_are_explicit() {
    use super::types::parse_lazer_line;
    let valid = "a1234567-89ab-cdef-0123-456789abcdef";
    for index in 0..valid.len() {
        let mut bytes = valid.as_bytes().to_vec();
        bytes[index] = b'g';
        let id = String::from_utf8(bytes).unwrap();
        let line = serde_json::json!({
            "type": "collection", "id": id, "name": "test", "beatmap_md5_hashes": [],
        })
        .to_string();
        assert!(parse_lazer_line(&line).is_err(), "accepted Guid {id}");
    }
    for kind in [
        serde_json::Value::Null,
        serde_json::json!(true),
        serde_json::json!(1),
        serde_json::json!("Collection"),
    ] {
        let line =
            serde_json::json!({ "type": kind, "source": "Lazer", "beatmaps": [] }).to_string();
        assert!(parse_lazer_line(&line).is_err(), "accepted type {kind}");
    }
}

#[cfg(unix)]
mod helper_process {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::Duration,
    };

    use tempfile::TempDir;
    use tokio::time::timeout;

    use super::super::scanner::{
        import_from_lazer_realm_with_helper, import_snapshot_from_lazer_realm_with_helper,
    };

    #[tokio::test]
    async fn default_and_compatibility_imports_honor_the_runtime_helper_and_validate_collections() {
        const FIXTURE: &str = "OSU_RADIO_TEST_DEFAULT_LAZER_HELPER";
        if let Some(mode) = std::env::var_os(FIXTURE) {
            let root = std::env::current_dir().unwrap();
            let marker = radio_core::OsuMarker {
                kind: radio_core::OsuKind::Lazer,
                marker_path: root.join("曲 with spaces.realm"),
                root_path: root,
            };
            let direct = crate::import_from_lazer_realm(&marker.marker_path).await;
            let compatibility = crate::get_beatmap_sets(marker.clone()).await;
            if mode == "valid" {
                for sets in [direct.unwrap(), compatibility.unwrap()] {
                    assert_eq!(sets.len(), 1);
                    assert_eq!(
                        sets[0].beatmaps[0]
                            .metadata
                            .as_ref()
                            .unwrap()
                            .title
                            .as_deref(),
                        Some("override")
                    );
                }
                let snapshot = crate::import_snapshot(marker).await.unwrap();
                assert_eq!(snapshot.beatmap_sets.len(), 1);
                assert_eq!(snapshot.collections.len(), 1);
                assert_eq!(snapshot.collections[0].name, "kept by snapshot");
            } else {
                // Set-only compatibility APIs must still validate collection records.
                for result in [direct, compatibility] {
                    assert!(result.unwrap_err().to_string().contains("collection"));
                }
            }
            return;
        }

        for (mode, collection) in [
            (
                "valid",
                r#"{"type":"collection","id":"a1234567-89ab-cdef-0123-456789abcdef","name":"kept by snapshot","beatmap_md5_hashes":[]}"#,
            ),
            (
                "invalid",
                r#"{"type":"collection","id":"invalid","name":"bad","beatmap_md5_hashes":[]}"#,
            ),
        ] {
            let (directory, helper) = fake_helper(&format!(
                "printf '%s\\n' '{{\"source\":\"Lazer\",\"beatmaps\":[{{\"metadata\":{{\"title\":\"override\"}}}}]}}' '{collection}'"
            ));
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.env_clear().args([
                "--exact",
                "lazer::tests::helper_process::default_and_compatibility_imports_honor_the_runtime_helper_and_validate_collections",
                "--nocapture",
            ]).env(FIXTURE, mode).env("OSU_LAZER_REALM_PARSER_PATH", helper).current_dir(directory.path());
            if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
                child.env("LLVM_PROFILE_FILE", profile);
            }
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "{mode}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[tokio::test]
    async fn relocated_executable_finds_the_installed_helper_and_explicit_override_wins() {
        const FIXTURE: &str = "OSU_RADIO_TEST_INSTALLED_LAZER_HELPER";
        const TEST: &str = "lazer::tests::helper_process::relocated_executable_finds_the_installed_helper_and_explicit_override_wins";
        if let Some(expected) = std::env::var_os(FIXTURE) {
            let result = crate::import_from_lazer_realm(Path::new("unused.realm")).await;
            let expected = expected.to_str().unwrap();
            match expected.strip_prefix("error:") {
                Some(message) => assert!(result.unwrap_err().to_string().contains(message)),
                None => assert_eq!(
                    result.unwrap()[0].beatmaps[0]
                        .metadata
                        .as_ref()
                        .unwrap()
                        .title
                        .as_deref(),
                    Some(expected)
                ),
            }
            return;
        }

        // A whole bundle in a spaced/Cyrillic path, run from an unrelated cwd.
        let bundle = TempDir::new().unwrap();
        let root = bundle.path().join("Приложение osu radio");
        let current = std::env::current_exe().unwrap();
        let executable = root.join("bin").join(current.file_name().unwrap());
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::copy(&current, &executable).unwrap();
        let helper = root.join("lib/osu-radio/osu-lazer-realm-parser");
        fs::create_dir_all(helper.parent().unwrap()).unwrap();
        let script = |title: &str| {
            format!(
                "#!/bin/sh\nprintf '%s\\n' '{{\"source\":\"Lazer\",\"beatmaps\":[{{\"metadata\":{{\"title\":\"{title}\"}}}}]}}'\n"
            )
        };
        fs::write(&helper, script("installed")).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let (_other, explicit) = fake_helper(script("explicit").trim_start_matches("#!/bin/sh\n"));
        let cwd = TempDir::new().unwrap();

        let run = |expected: &str, helper_env: Option<&Path>| {
            let mut child = std::process::Command::new(&executable);
            child
                .env_clear()
                .args(["--exact", TEST, "--nocapture"])
                .env(FIXTURE, expected)
                .current_dir(cwd.path());
            if let Some(helper_env) = helper_env {
                child.env("OSU_LAZER_REALM_PARSER_PATH", helper_env);
            }
            if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
                child.env("LLVM_PROFILE_FILE", profile);
            }
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "{expected}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run("installed", None);
        run("explicit", Some(&explicit));
        // A broken explicit override is an error even though a working installed helper exists.
        run("error:failed to start", Some(&root.join("missing helper")));
        run("error:set but empty", Some(Path::new("")));
    }

    #[tokio::test]
    async fn accepts_a_fake_ndjson_helper() {
        let (_directory, helper) = fake_helper(
            "printf '%s\\n' '{\"source\":\"Lazer\",\"beatmaps\":[{\"metadata\":{\"title\":\"test\"}}]}'",
        );
        let result = import_from_lazer_realm_with_helper(Path::new("unused.realm"), &helper).await;
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn includes_helper_stderr_in_failure() {
        let (_directory, helper) = fake_helper("echo 'realm exploded' >&2\nexit 7");
        let error = import_from_lazer_realm_with_helper(Path::new("unused.realm"), &helper)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("realm exploded"));
    }

    #[tokio::test]
    async fn passes_one_exact_realm_argument_and_preserves_mixed_record_order() {
        let (directory, helper) = fake_helper(concat!(
            "printf '%s' \"$#\" > \"$1.argc\"\n",
            "printf '%s' \"$1\" > \"$1.argument\"\n",
            "printf '%s\\r\\n' '{\"source\":\"Lazer\",\"beatmaps\":[{\"difficulty_name\":\"First\"}]}'\n",
            "printf '%s\\n' '{\"type\":\"collection\",\"id\":\"A1234567-89AB-CDEF-0123-456789ABCDEF\",\"name\":\"曲 🎵\",\"beatmap_md5_hashes\":[]}'\n",
            "printf '%s' '{\"source\":\"Lazer\",\"beatmaps\":[{\"difficulty_name\":\"Last\"}]}'\n",
        ));
        let realm = directory.path().join("space and 曲.realm");
        let snapshot = crate::import_snapshot_with_helper(
            radio_core::OsuMarker {
                kind: radio_core::OsuKind::Lazer,
                root_path: directory.path().to_path_buf(),
                marker_path: realm.clone(),
            },
            helper,
        )
        .await
        .unwrap();
        assert_eq!(
            fs::read_to_string(realm.with_extension("realm.argc")).unwrap(),
            "1"
        );
        assert_eq!(
            fs::read_to_string(realm.with_extension("realm.argument")).unwrap(),
            realm.to_str().unwrap()
        );
        assert_eq!(snapshot.beatmap_sets.len(), 2);
        assert_eq!(
            snapshot.beatmap_sets[0].beatmaps[0]
                .difficulty_name
                .as_deref(),
            Some("First")
        );
        assert_eq!(
            snapshot.beatmap_sets[1].beatmaps[0]
                .difficulty_name
                .as_deref(),
            Some("Last")
        );
        assert_eq!(snapshot.collections.len(), 1);
        assert_eq!(snapshot.collections[0].name, "曲 🎵");
        assert_eq!(
            snapshot.collections[0].source_id,
            "a1234567-89ab-cdef-0123-456789abcdef"
        );
    }

    #[tokio::test]
    async fn empty_success_is_an_empty_snapshot_but_nonzero_exit_discards_valid_records() {
        for (body, succeeds) in [
            ("exit 0", true),
            (
                "printf '%s\\n' '{\"source\":\"Lazer\",\"beatmaps\":[]}'\nexit 7",
                false,
            ),
        ] {
            let (_directory, helper) = fake_helper(body);
            let result =
                import_snapshot_from_lazer_realm_with_helper(Path::new("unused.realm"), helper)
                    .await;
            if succeeds {
                let snapshot = result.unwrap();
                assert!(snapshot.beatmap_sets.is_empty());
                assert!(snapshot.collections.is_empty());
            } else {
                assert!(result.unwrap_err().to_string().contains("exited with"));
            }
        }
    }

    #[tokio::test]
    async fn drains_more_than_one_pipe_buffer_of_stderr_before_stdout_completes() {
        let (_directory, helper) = fake_helper(concat!(
            "printf '%0200000d' 0 >&2\n",
            "printf '%s\\n' '{\"source\":\"Lazer\",\"beatmaps\":[]}'\n",
            "printf '%s' 'diagnostic-tail' >&2\nexit 7",
        ));
        let error = timeout(
            Duration::from_secs(5),
            import_from_lazer_realm_with_helper(Path::new("unused.realm"), helper),
        )
        .await
        .unwrap()
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("exited with"));
        assert!(message.len() > 200_000);
        assert!(message.ends_with("diagnostic-tail"));
    }

    #[tokio::test]
    async fn invalid_stderr_encoding_rejects_otherwise_valid_output() {
        let (_directory, helper) = fake_helper(
            "printf '%s\\n' '{\"source\":\"Lazer\",\"beatmaps\":[]}'\nprintf '\\377\\n' >&2",
        );
        let error = import_from_lazer_realm_with_helper(Path::new("unused.realm"), helper)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to read Realm helper stderr"),
            "{error:#}"
        );
    }

    #[tokio::test]
    async fn spawn_failure_identifies_both_helper_and_realm_paths() {
        let directory = TempDir::new().unwrap();
        let helper = directory.path().join("missing helper");
        let realm = directory.path().join("曲.realm");
        let error = import_from_lazer_realm_with_helper(&realm, &helper)
            .await
            .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("failed to start"));
        assert!(message.contains(helper.to_str().unwrap()));
        assert!(message.contains(realm.to_str().unwrap()));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn invalid_stdout_kills_and_reaps_the_helper_before_returning_an_error() {
        for (output, expected) in [
            ("printf '%s\\n' 'not-json'", "invalid Realm helper JSON"),
            ("printf '\\n'", "invalid Realm helper JSON"),
            (
                "printf '%s\\n' '{\"type\":\"schema\"}'",
                "unknown Realm helper record type",
            ),
            ("printf '\\377\\n'", "failed to read Realm helper stdout"),
        ] {
            let (directory, helper) = fake_helper(&format!(
                "printf '%s' \"$$\" > \"$1.pid\"\nprintf '%s\\n' '{{\"source\":\"Lazer\",\"beatmaps\":[]}}'\n{output}\nexec sleep 30"
            ));
            let realm = directory.path().join("client.realm");
            let error = timeout(
                Duration::from_secs(5),
                import_snapshot_from_lazer_realm_with_helper(&realm, helper),
            )
            .await
            .unwrap()
            .unwrap_err();
            assert!(error.to_string().contains(expected), "{error:#}");
            let pid = fs::read_to_string(realm.with_extension("realm.pid")).unwrap();
            assert!(
                !Path::new(&format!("/proc/{pid}")).exists(),
                "helper {pid} was not reaped"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn cancelling_an_import_terminates_its_live_helper() {
        let (directory, helper) = fake_helper("printf '%s' \"$$\" > \"$1.pid\"\nexec sleep 30");
        let realm = directory.path().join("client.realm");
        let pid_path = realm.with_extension("realm.pid");
        let task = tokio::spawn(async move {
            import_snapshot_from_lazer_realm_with_helper(&realm, helper).await
        });
        let pid = timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(pid) = fs::read_to_string(&pid_path)
                    && !pid.is_empty()
                {
                    break pid;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        // Abort even if the readiness deadline fails, so a failed test cannot detach the import.
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let pid = pid.unwrap();
        timeout(Duration::from_secs(5), async {
            while Path::new(&format!("/proc/{pid}")).exists() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled helper must be killed and reaped");
    }

    fn fake_helper(body: &str) -> (TempDir, PathBuf) {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("realm helper");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();

        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).unwrap();
        (directory, path)
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

fn real_realm_fixtures() -> (tempfile::TempDir, std::path::PathBuf) {
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
    let fixtures = temp.path().join("fixtures 曲 with spaces");
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
    (temp, fixtures)
}

#[tokio::test]
async fn imports_real_realm_fixtures_with_the_production_helper() {
    use super::scanner::import_snapshot_from_lazer_realm_with_helper;
    use std::process::Command;

    let (_temp, fixtures) = real_realm_fixtures();
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
    assert!(direct.stderr.is_empty());
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
    assert!(without.beatmap_sets[0].files.is_empty());
    for map in &without.beatmap_sets[0].beatmaps {
        assert_eq!(map.bpm, None);
        assert!(map.metadata.is_none());
    }
    for filename in ["invalid-md5.realm", "missing-name.realm"] {
        assert!(
            import_snapshot_from_lazer_realm_with_helper(&fixtures.join(filename), helper)
                .await
                .is_err(),
            "{filename}"
        );
    }
}

#[tokio::test]
async fn real_realm_metadata_and_file_references_round_trip_without_modifying_the_database() {
    use super::scanner::import_snapshot_from_lazer_realm_with_helper;

    let (_temp, fixtures) = real_realm_fixtures();
    let path = fixtures.join("metadata.realm");
    let before = std::fs::read(&path).unwrap();
    let snapshot = import_snapshot_from_lazer_realm_with_helper(
        &path,
        env!("OSU_LAZER_REALM_PARSER_BUILT_PATH"),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(snapshot.collections.is_empty());
    assert_eq!(snapshot.beatmap_sets.len(), 1);
    let set = &snapshot.beatmap_sets[0];
    assert_eq!(set.online_id, None, "overflowing Realm long must not wrap");
    assert_eq!(set.hash.as_deref(), Some("rich-set"));
    assert_eq!(set.beatmaps.len(), 4);
    let map = &set.beatmaps[0];
    assert_eq!(map.difficulty_name.as_deref(), Some("Hard"));
    assert_eq!(map.bpm, Some(180.5));
    assert_eq!(map.hash, None);
    assert_eq!(map.md5_hash, None);
    let metadata = map.metadata.as_ref().unwrap();
    assert_eq!(metadata.title.as_deref(), Some("Song"));
    assert_eq!(metadata.title_unicode.as_deref(), Some("曲 🎵"));
    assert_eq!(metadata.artist.as_deref(), Some("Artist"));
    assert_eq!(metadata.artist_unicode.as_deref(), Some("演奏者"));
    assert_eq!(metadata.source.as_deref(), Some("Game"));
    assert_eq!(metadata.tags.as_deref(), Some("one two"));
    assert_eq!(metadata.user_tags, ["favorite", "曲"]);
    assert_eq!(metadata.preview_time, Some(-123));
    assert_eq!(metadata.audio_file.as_deref(), Some("audio.mp3"));
    assert_eq!(metadata.background_file.as_deref(), Some("cover.jpg"));
    let author = metadata.author.as_ref().unwrap();
    assert_eq!(author.online_id, Some(456));
    assert_eq!(author.username.as_deref(), Some("Mapper"));
    assert_eq!(author.country_code.as_deref(), Some("EE"));
    let empty = set.beatmaps[1].metadata.as_ref().unwrap();
    assert_eq!(empty.preview_time, None);
    assert_eq!(empty.author.as_ref().unwrap().online_id, None);
    assert!(empty.user_tags.is_empty());
    assert!(set.beatmaps[2].metadata.is_none());
    assert!(set.beatmaps[3].metadata.as_ref().unwrap().author.is_none());
    assert_eq!(set.files.len(), 7);
    assert_eq!(
        set.resolved_audio_path(map),
        Some(fixtures.join("files/a/ab/abcdef1234").as_path())
    );
    assert_eq!(
        set.resolved_background_path(map),
        Some(fixtures.join("files/1/12/12cover").as_path())
    );
    for usage in &set.files[2..5] {
        assert!(usage.file.as_ref().unwrap().resolved_path.is_none());
    }
    assert_eq!(
        set.files[2].file.as_ref().unwrap().hash.as_deref(),
        Some("x")
    );
    assert_eq!(
        set.files[3].file.as_ref().unwrap().hash.as_deref(),
        Some("")
    );
    assert!(set.files[4].file.as_ref().unwrap().hash.is_none());
    assert!(set.files[5].file.is_none());
    assert_eq!(
        set.files[6].file.as_ref().unwrap().resolved_path,
        Some(fixtures.join("files/1/12/12"))
    );
    assert!(
        !fixtures.join("files").exists(),
        "import must not create media"
    );
}

#[test]
fn real_helper_cli_errors_keep_diagnostics_off_stdout_and_preserve_input_bytes() {
    use std::process::Command;

    let helper = env!("OSU_LAZER_REALM_PARSER_BUILT_PATH");
    let temp = tempfile::TempDir::new().unwrap();
    for args in [vec![], vec!["--unknown", "file.realm"], vec!["a", "b", "c"]] {
        let output = Command::new(helper)
            .args(&args)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("Usage:"));
    }
    for path in [
        temp.path().join("missing 曲.realm"),
        temp.path().to_path_buf(),
    ] {
        let output = Command::new(helper).arg(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(3));
        assert!(output.stdout.is_empty());
        let message = String::from_utf8(output.stderr).unwrap();
        assert!(message.contains("Realm file does not exist:"));
        assert!(message.contains(&path.display().to_string()));
    }
    let path = temp.path().join("invalid.realm");
    std::fs::write(&path, b"not a Realm file").unwrap();
    for schema in [false, true] {
        let mut command = Command::new(helper);
        if schema {
            command.arg("--schema");
        }
        let output = command.arg(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(4));
        assert!(output.stdout.is_empty());
        let message = String::from_utf8(output.stderr).unwrap();
        assert!(message.contains("Failed to read Realm"));
        assert!(message.contains(&path.display().to_string()));
        assert_eq!(std::fs::read(&path).unwrap(), b"not a Realm file");
    }
}

#[test]
fn real_helper_schema_is_sorted_ndjson_with_typed_fields_and_no_database_changes() {
    use std::process::Command;

    let (_temp, fixtures) = real_realm_fixtures();
    let helper = env!("OSU_LAZER_REALM_PARSER_BUILT_PATH");
    let path = fixtures.join("metadata.realm");
    let before = std::fs::read(&path).unwrap();
    let output = Command::new(helper)
        .arg("--schema")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let records: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let names: Vec<_> = records
        .iter()
        .map(|row| row["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "Author",
            "Beatmap",
            "BeatmapSet",
            "FileEntry",
            "Metadata",
            "NamedFileUsage",
            "SchemaExtras"
        ]
    );
    assert!(
        records
            .iter()
            .all(|row| row["type"] == "schema" && row["objectType"] == "RealmObject")
    );
    let extras = records
        .iter()
        .find(|row| row["name"] == "SchemaExtras")
        .unwrap();
    let fields = extras["fields"].as_array().unwrap();
    for (name, collection, kind, nullable) in [
        ("Items", serde_json::json!("list"), "String", false),
        ("Tags", serde_json::json!("set"), "String", false),
        ("Metrics", serde_json::json!("dictionary"), "Int", true),
        ("OptionalNumber", serde_json::Value::Null, "Int", true),
    ] {
        let field = fields.iter().find(|field| field["name"] == name).unwrap();
        assert_eq!(field["collection"], collection, "{field}");
        assert_eq!(field["propertyType"], kind, "{field}");
        assert_eq!(field["nullable"], nullable, "{field}");
        assert_eq!(field["managedName"], name);
        assert!(field["objectType"].is_null());
        assert!(field["linkOriginProperty"].is_null());
    }
    assert!(
        fields
            .iter()
            .any(|field| field["name"] == "ID" && field["primaryKey"] == true)
    );
    assert!(
        fields
            .iter()
            .any(|field| field["name"] == "Label" && field["indexType"] == "General")
    );
    let set = records
        .iter()
        .find(|row| row["name"] == "BeatmapSet")
        .unwrap();
    let beatmaps = set["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["name"] == "Beatmaps")
        .unwrap();
    assert_eq!(beatmaps["propertyType"], "Object");
    assert_eq!(beatmaps["objectType"], "Beatmap");
    assert_eq!(beatmaps["collection"], "list");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn real_helper_accepts_an_unrelated_schema_only_in_inspection_mode() {
    use std::process::Command;

    let (_temp, fixtures) = real_realm_fixtures();
    let helper = env!("OSU_LAZER_REALM_PARSER_BUILT_PATH");
    let missing = fixtures.join("missing-beatmap-set.realm");
    let before = std::fs::read(&missing).unwrap();
    let output = Command::new(helper).arg(&missing).output().unwrap();
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("expected 'BeatmapSet'")
    );
    let schema = Command::new(helper)
        .arg("--schema")
        .arg(&missing)
        .output()
        .unwrap();
    assert!(schema.status.success());
    let row: serde_json::Value = serde_json::from_slice(&schema.stdout).unwrap();
    assert_eq!(row["name"], "SchemaExtras");
    assert_eq!(std::fs::read(&missing).unwrap(), before);
}

#[test]
fn helper_selection_order_is_override_then_installed_then_built() {
    use super::scanner::{INSTALLED_HELPER_PATH, select_helper};
    let bundle = tempfile::TempDir::new().unwrap();
    let executable = bundle.path().join("bin/app");
    let installed = executable.parent().unwrap().join(INSTALLED_HELPER_PATH);
    let select = |helper: Option<&str>, exe: Option<&Path>| {
        select_helper(helper.map(Into::into), exe, "/build/helper")
    };

    assert_eq!(
        select(None, Some(&executable)).unwrap(),
        Path::new("/build/helper")
    );
    assert_eq!(select(None, None).unwrap(), Path::new("/build/helper"));

    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::write(&installed, b"").unwrap();
    assert_eq!(select(None, Some(&executable)).unwrap(), installed);
    assert_eq!(
        select(Some("/chosen"), Some(&executable)).unwrap(),
        Path::new("/chosen")
    );
    // A missing explicit override stays explicit; an empty one is rejected up front.
    assert_eq!(
        select(Some("/missing"), Some(&executable)).unwrap(),
        Path::new("/missing")
    );
    assert!(
        select(Some(""), Some(&executable))
            .unwrap_err()
            .to_string()
            .contains("empty")
    );
}
