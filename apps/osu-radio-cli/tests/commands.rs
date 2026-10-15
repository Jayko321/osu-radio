#![allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]

mod support;

use std::fs;

use support::{Fixture, failure, json, stdout};

#[test]
fn no_command_and_help_explain_the_cli_without_accessing_storage() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.root.join(".env")).unwrap();
    let default = stdout(&fixture.run(&[]));
    assert_eq!(default, stdout(&fixture.run(&["help"])));
    assert!(default.contains("development CLI"));
    assert!(stdout(&fixture.run(&["--help"])).contains("scan"));
    assert!(stdout(&fixture.run(&["--version"])).contains(env!("CARGO_PKG_VERSION")));
    assert!(!fixture.root.join("library.sqlite").exists());
}

#[test]
fn scan_json_filters_sources_and_resolves_relative_roots() {
    let fixture = Fixture::new();
    let stable = fixture.install("sources/stable", &[]);
    let lazer = fixture.root.join("sources/lazer");
    fs::create_dir(&lazer).unwrap();
    fs::write(lazer.join("client.realm"), b"discovery only").unwrap();
    let value = json(&fixture.run(&["scan", "--root", "sources", "--depth", "full", "--json"]));
    let markers = value["markers"].as_array().unwrap();
    assert_eq!(markers.len(), 2);
    for (index, marker) in markers.iter().enumerate() {
        assert_eq!(marker["index"], index.saturating_add(1));
    }
    for (source, root, filename) in [
        ("stable", &stable, "osu!.db"),
        ("lazer", &lazer, "client.realm"),
    ] {
        let filtered =
            json(&fixture.run(&["scan", "--root", "sources", "--source", source, "--json"]));
        assert_eq!(
            filtered["markers"],
            serde_json::json!([{
                "index": 1, "source": source,
                "root_path": root.display().to_string(),
                "marker_path": root.join(filename).display().to_string(),
            }])
        );
    }
    for limit in ["--first", "-1"] {
        let value = json(&fixture.run(&["scan", "--root", "sources", limit, "--json"]));
        assert_eq!(value["markers"].as_array().unwrap().len(), 1);
    }
    let value = json(&fixture.run(&["scan", "--root", "sources", "--limit", "1", "--json"]));
    assert_eq!(value["markers"].as_array().unwrap().len(), 1);
}

#[test]
fn scan_text_reports_empty_results_streamed_rows_and_verbose_selection_hint() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("sources")).unwrap();
    let output = fixture.run(&["scan", "--root", "sources"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("No osu! installations found"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Searching"));
    let stable = fixture.install("sources/osu!", &[]);
    let output = fixture.run(&["scan", "--root", "sources", "--depth", "known", "--verbose"]);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Index  Source"));
    assert!(text.contains("stable"));
    assert_eq!(
        text.lines()
            .nth(2)
            .unwrap()
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>(),
        ["1", "stable"]
    );
    assert!(text.contains("Found 1 osu! installation(s)."));
    assert!(text.contains("Indexes are 1-based"));
    assert!(stable.join("osu!.db").exists());
    assert!(!fixture.root.join("library.sqlite").exists());
}

#[test]
fn scan_rejects_zero_limits_and_clap_reports_invalid_flags() {
    let fixture = Fixture::new();
    failure(
        &fixture.run(&["scan", "--root", "missing", "--limit", "0"]),
        "`--limit` must be at least 1",
    );
    for args in [
        vec!["scan", "--first", "--limit", "1"],
        vec!["store", "--count", "1"],
    ] {
        let output = fixture.run(&args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn import_json_limits_difficulties_across_sets_without_touching_the_source() {
    let fixture = Fixture::new();
    let root = fixture.install(
        "osu",
        &[("set-a", "Easy"), ("set-a", "Hard"), ("set-b", "Expert")],
    );
    let original = fs::read(root.join("osu!.db")).unwrap();
    for (limit, set_lengths) in [
        (0, vec![]),
        (1, vec![1]),
        (2, vec![2]),
        (3, vec![2, 1]),
        (10, vec![2, 1]),
    ] {
        let value = json(&fixture.run(&[
            "import",
            "--marker",
            "osu/osu!.db",
            "--json",
            "--limit",
            &limit.to_string(),
        ]));
        assert_eq!(value["total_beatmap_sets"], 2);
        assert_eq!(value["total_beatmaps"], 3);
        assert_eq!(value["shown_beatmaps"], limit.min(3));
        let sets = value["beatmap_sets"].as_array().unwrap();
        assert_eq!(
            sets.iter()
                .map(|set| set["beatmaps"].as_array().unwrap().len())
                .collect::<Vec<_>>(),
            set_lengths
        );
        if limit > 0 {
            let beatmap = &sets[0]["beatmaps"][0];
            assert_eq!(beatmap["metadata"]["title_unicode"], "曲");
            assert_eq!(beatmap["difficulty_name"], "Easy");
            assert_eq!(beatmap["bpm"], 120.0);
            assert_eq!(
                beatmap["resolved_audio_path"],
                root.join("Songs/set-a/audio.mp3").display().to_string()
            );
        }
    }
    assert_eq!(fs::read(root.join("osu!.db")).unwrap(), original);
    assert_eq!(
        fs::read(root.join("Songs/set-a/audio.mp3")).unwrap(),
        b"source audio stays intact"
    );
    assert!(!fixture.root.join("library.sqlite").exists());
}

#[test]
fn import_table_shows_unicode_bpm_audio_and_preview_limit() {
    let fixture = Fixture::new();
    fixture.install("osu", &[("set-a", "Easy"), ("set-a", "Hard")]);
    let text = stdout(&fixture.run(&[
        "import",
        "--marker",
        "osu/osu!.db",
        "--limit",
        "1",
        "--verbose",
    ]));
    for expected in [
        "1 beatmap set(s) and 2 beatmap(s)",
        "歌手",
        "曲",
        "Easy",
        "120.0",
        "resolved",
        "Showing first 1 of 2",
        "Audio status is based",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert!(!text.contains("Hard"));
    let text = stdout(&fixture.run(&["import", "--marker", "osu/osu!.db", "--limit", "0"]));
    assert!(text.contains("2 beatmap(s)"));
    assert!(!text.contains("Difficulty"));
    fixture.install("empty", &[]);
    assert!(
        stdout(&fixture.run(&["import", "--marker", "empty/osu!.db"]))
            .contains("0 beatmap set(s) and 0 beatmap(s)")
    );
}

#[test]
fn marker_selection_handles_none_unique_ambiguous_and_one_based_indexes() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("sources")).unwrap();
    failure(
        &fixture.run(&["import", "--root", "sources"]),
        "No osu! installations found",
    );
    fixture.install("sources/a", &[]);
    assert_eq!(
        json(&fixture.run(&["import", "--root", "sources", "--json"]))["total_beatmaps"],
        0
    );
    assert_eq!(
        json(&fixture.run(&["import", "--root", "sources", "--index", "1", "--json"]))["total_beatmaps"],
        0
    );
    failure(
        &fixture.run(&["import", "--root", "sources", "--index", "0"]),
        "`--index` is 1-based",
    );
    failure(
        &fixture.run(&["import", "--root", "sources", "--index", "2"]),
        "No discovered osu! installation has index 2",
    );
    fixture.install("sources/b", &[]);
    failure(
        &fixture.run(&["import", "--root", "sources"]),
        "Multiple osu! installations found",
    );
    // Discovery order is intentionally unspecified; either empty installation may be second.
    assert_eq!(
        json(&fixture.run(&["import", "--root", "sources", "--index", "2", "--json"]))["total_beatmaps"],
        0
    );
    failure(
        &fixture.run(&["import", "--index", "1", "--marker", "sources/a/osu!.db"]),
        "Use either `--index` or `--marker`",
    );
}

#[test]
fn explicit_marker_errors_include_source_path_and_reader_cause() {
    let fixture = Fixture::new();
    failure(
        &fixture.run(&["import", "--marker", "unknown.db"]),
        "Could not infer osu! source",
    );
    failure(
        &fixture.run(&["import", "--marker", "missing/osu!.db"]),
        "Failed to import beatmaps from stable marker",
    );
    fs::write(fixture.root.join("custom.db"), b"broken database").unwrap();
    let output = fixture.run(&["import", "--marker", "custom.db", "--source", "stable"]);
    failure(&output, "custom.db");
    assert!(String::from_utf8_lossy(&output.stderr).contains("byte offset"));
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn lazer_helper_override_is_child_scoped_and_forwards_success_diagnostics() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    let helper = fixture.root.join("realm helper");
    fs::write(&helper, concat!(
        "#!/bin/sh\nprintf '%s\\n' 'helper diagnostic' >&2\n",
        "printf '%s\\n' '{\"source\":\"Lazer\",\"beatmaps\":[{\"difficulty_name\":\"Hard\"}]}'\n",
    )).unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(fixture.root.join("client.realm"), b"source stays intact").unwrap();
    let output = fixture
        .command()
        .args(["import", "--marker", "client.realm", "--json"])
        .env("OSU_LAZER_REALM_PARSER_PATH", helper)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stderr, b"helper diagnostic\n");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["marker"]["source"], "lazer");
    assert_eq!(value["total_beatmaps"], 1);
    assert_eq!(
        value["beatmap_sets"][0]["beatmaps"][0]["difficulty_name"],
        "Hard"
    );
    assert_eq!(
        fs::read(fixture.root.join("client.realm")).unwrap(),
        b"source stays intact"
    );
    assert!(!fixture.root.join("library.sqlite").exists());
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn store_replaces_only_the_selected_installation_until_clear_is_explicit() {
    let fixture = Fixture::new();
    let source = fixture.install(
        "osu",
        &[("set-a", "Easy"), ("set-a", "Hard"), ("set-b", "Expert")],
    );
    let output = json(&fixture.run(&["store", "--marker", "osu/osu!.db", "--json"]));
    assert_eq!(output["cleared"], false);
    assert_eq!(output["discovered_beatmap_sets"], 2);
    assert_eq!(output["discovered_beatmaps"], 3);
    assert_eq!(output["stored_beatmap_sets"], 2);
    assert_eq!(output["stored_beatmaps"], 3);
    assert_eq!(output["stored_audio_sources"], 2);
    let id = output["installation_id"].as_i64().unwrap();
    let duplicate = json(&fixture.run(&["store", "--marker", "osu/osu!.db", "--json"]));
    assert_eq!(duplicate["installation_id"], id);
    assert_eq!(duplicate["stored_beatmaps"], 3);

    fixture.install("second", &[("set-c", "Another")]);
    let summary = stdout(&fixture.run(&["store", "--marker", "second/osu!.db", "--verbose"]));
    assert!(summary.contains("Registered osu! folder"));
    assert!(summary.contains("nothing was copied"));
    assert!(summary.contains("complete library snapshot"));
    fixture.install("osu", &[]);
    let summary = stdout(&fixture.run(&["store", "--marker", "osu/osu!.db"]));
    assert!(summary.contains("already registered"));
    assert!(summary.contains("Stored 0 beatmap set(s), 0 beatmap(s)"));

    let database_path = fixture.root.join("library.sqlite");
    {
        let services = radio_services::Services::connect(database_path.to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(services.osu_installations().all().await.unwrap().len(), 2);
        let sets = services
            .beatmap_sets()
            .all_with_audio_sources()
            .await
            .unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(
            sets[0].beatmaps[0].difficulty_name.as_deref(),
            Some("Another")
        );
        assert_eq!(sets[0].audio_sources.len(), 1);
        assert_eq!(sets[0].audio_sources[0].s_type.kind(), "local");
        assert_eq!(
            sets[0].audio_sources[0].s_type.location(),
            fixture
                .root
                .join("second/Songs/set-c/audio.mp3")
                .to_str()
                .unwrap()
        );
    }
    let summary = stdout(&fixture.run(&["store", "--marker", "osu/osu!.db", "--clear"]));
    assert!(summary.contains("Cleared every previously stored beatmap"));
    {
        let services = radio_services::Services::connect(database_path.to_str().unwrap())
            .await
            .unwrap();
        let installations = services.osu_installations().all().await.unwrap();
        assert_eq!(installations.len(), 1);
        assert_eq!(installations[0].root_path, std::path::Path::new("osu"));
        assert!(
            services
                .beatmap_sets()
                .all_with_audio_sources()
                .await
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        fs::read(source.join("Songs/set-a/audio.mp3")).unwrap(),
        b"source audio stays intact"
    );
    assert_eq!(
        fs::read(source.join("Songs/set-a/cover.jpg")).unwrap(),
        b"source artwork stays intact"
    );
}

#[cfg(feature = "sqlite")]
#[test]
fn store_reads_the_complete_source_before_clear_or_database_creation() {
    let fixture = Fixture::new();
    fixture.install("osu", &[("set-a", "Hard")]);
    json(&fixture.run(&["store", "--marker", "osu/osu!.db", "--json"]));
    let database = fixture.root.join("library.sqlite");
    let original = fs::read(&database).unwrap();
    fs::write(
        fixture.root.join("osu/collection.db"),
        b"corrupt collections",
    )
    .unwrap();
    failure(
        &fixture.run(&["store", "--marker", "osu/osu!.db", "--clear"]),
        "Failed to import beatmaps",
    );
    assert_eq!(fs::read(&database).unwrap(), original);
    fs::remove_file(fixture.root.join("osu/collection.db")).unwrap();
    fs::write(fixture.root.join("osu/osu!.db"), b"corrupt").unwrap();
    failure(
        &fixture.run(&["store", "--marker", "osu/osu!.db", "--clear"]),
        "Failed to import beatmaps",
    );
    assert_eq!(fs::read(&database).unwrap(), original);
    fs::remove_file(&database).unwrap();
    failure(
        &fixture.run(&["store", "--marker", "osu/osu!.db", "--clear"]),
        "Failed to import beatmaps",
    );
    assert!(!database.exists());
}

#[cfg(feature = "sqlite")]
#[test]
fn database_uses_isolated_dotenv_and_reports_configuration_failures() {
    let fixture = Fixture::new();
    assert!(stdout(&fixture.run(&["database"])).contains("Connected to the database"));
    assert!(fixture.root.join("library.sqlite").exists());
    fs::remove_file(fixture.root.join("library.sqlite")).unwrap();
    let output = fixture
        .command()
        .arg("database")
        .env("SQLITE_DATABASE_URL", ":memory:")
        .output()
        .unwrap();
    assert!(stdout(&output).contains("Connected to the database"));
    assert!(!fixture.root.join("library.sqlite").exists());
    fs::write(fixture.root.join(".env"), "# missing database URL\n").unwrap();
    failure(
        &fixture.run(&["database"]),
        "SQLITE_DATABASE_URL must be set",
    );
    fs::write(
        fixture.root.join(".env"),
        "SQLITE_DATABASE_URL=missing/library.sqlite\n",
    )
    .unwrap();
    failure(
        &fixture.run(&["database"]),
        "Failed to connect to the configured database",
    );
    fs::write(fixture.root.join(".env"), [0xff]).unwrap();
    failure(&fixture.run(&["database"]), "Failed to load .env");
}
