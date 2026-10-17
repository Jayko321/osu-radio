//! Opt-in measurements of captured HTTP payloads, without GUI rendering or media I/O.
#![allow(clippy::indexing_slicing)]
use super::*;
use crate::{BeatmapSet, models::BeatmapDetails};
use std::time::Instant;

#[cfg(unix)]
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn benchmark_payloads_and_http_paths_run_on_isolated_synthetic_inputs() {
    use std::io::{BufRead, Write};
    use std::os::unix::fs::PermissionsExt;
    const TEST: &str = "controller::benchmarks::benchmark_payloads_and_http_paths_run_on_isolated_synthetic_inputs";
    const SERVER: &str = "OSU_RADIO_TEST_BENCH_SERVER";
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    if std::env::var_os(SERVER).is_some() {
        let listener =
            std::net::TcpListener::bind(std::env::var("OSU_RADIO_SERVER_ADDRESS").unwrap())
                .unwrap();
        println!("\nlistening on http://{}", listener.local_addr().unwrap());
        std::io::stdout().flush().unwrap();
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = String::new();
            let mut reader = std::io::BufReader::new(&mut stream);
            reader.read_line(&mut request).unwrap();
            for header in reader.lines() {
                if header.unwrap().is_empty() {
                    break;
                }
            }
            let url = reqwest::Url::parse(&format!(
                "http://fixture{}",
                request.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            let query = url
                .query_pairs()
                .find(|(key, _)| key == "q")
                .unwrap()
                .1
                .into_owned();
            let index = ["rock", "rock hard", "星", "does-not-exist-zzzz", ""]
                .iter()
                .position(|expected| *expected == query)
                .unwrap();
            let prefix = match url.path() {
                "/api/beatmap-sets" => "baseline",
                "/api/tracks" => "stage3",
                path => panic!("unexpected benchmark path: {path}"),
            };
            let suffix = if prefix == "baseline" {
                "beatmap-sets"
            } else {
                "tracks"
            };
            let bytes = std::fs::read(format!("{prefix}-{profile}-{suffix}-{index}.json")).unwrap();
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("requests.jsonl")
                .unwrap();
            writeln!(log, "{}", serde_json::json!([url.path(), query])).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len()).unwrap();
            stream.write_all(&bytes).unwrap();
        }
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let rich = serde_json::json!([
        {"id": 1, "audio_sources": [{"id":10,"kind":"local","location":"/inert/a"},{"id":20,"kind":"local","location":"/inert/b"}], "beatmaps": [
            {"id":3,"audio_source_id":10,"title":"Later","artist":"Later","difficulty_name":"Easy","has_cover":true},
            {"id":1,"audio_source_id":10,"title":"Rock","title_unicode":"曲","artist_unicode":"作曲者","difficulty_name":"Hard","has_cover":false}
        ]},
        {"id": 2, "audio_sources": [{"id":10,"kind":"local","location":"/inert/a"},{"id":30,"kind":"local","location":"/inert/c"}], "beatmaps": [
            {"id":4,"audio_source_id":10,"difficulty_name":"Hard","has_cover":false},
            {"id":5,"audio_source_id":30,"has_cover":false}
        ]}
    ]);
    for (index, fixture) in [
        rich.clone(),
        serde_json::json!([rich[0]]),
        serde_json::json!([rich[1]]),
        serde_json::json!([]),
        rich,
    ]
    .into_iter()
    .enumerate()
    {
        let sets: Vec<BeatmapSet> = serde_json::from_value(fixture.clone()).unwrap();
        let tracks = crate::view_models::library_tracks(&sets);
        if index == 0 {
            let original = original_library_tracks(&sets);
            assert_eq!(
                original
                    .iter()
                    .map(|track| track.audio_source_id)
                    .collect::<Vec<_>>(),
                [10, 20, 30]
            );
            assert_eq!(original[0].title, "Rock");
            assert_eq!(original[0].subtitle, "作曲者 | Hard, Easy");
            assert_eq!(original[0].cover_beatmap_id, Some(3));
            assert_eq!(original[1].title, "Unknown title");
            assert_eq!(original[2].subtitle, "Unknown artist | Unknown difficulty");
        }
        let compact: Vec<_> = tracks.iter().map(|track| serde_json::json!({
            "audio_source_id":track.audio_source_id, "title":track.title_original, "title_unicode":track.title_unicode,
            "artist":track.artist_original, "artist_unicode":track.artist_unicode, "cover_beatmap_id":track.cover_beatmap_id,
            "difficulties":track.difficulties.iter().map(|difficulty| serde_json::json!({
                "beatmap_id":difficulty.beatmap_id,"beatmap_set_id":difficulty.beatmap_set_id,
                "difficulty_name":difficulty.difficulty_name,"set_has_multiple_audio_sources":difficulty.set_has_multiple_audio_sources,
            })).collect::<Vec<_>>()
        })).collect();
        std::fs::write(
            root.join(format!("baseline-{profile}-beatmap-sets-{index}.json")),
            serde_json::to_vec(&fixture).unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join(format!("stage3-{profile}-tracks-{index}.json")),
            serde_json::to_vec(&compact).unwrap(),
        )
        .unwrap();
    }
    let executable = std::env::current_exe().unwrap();
    let server = root.join("server");
    std::fs::write(
        &server,
        format!(
            "#!/bin/sh\n{SERVER}=1 exec '{}' --exact '{TEST}' --nocapture\n",
            executable.to_str().unwrap().replace('\'', "'\\''")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700)).unwrap();
    for (test, compact) in [
        ("captured_client_timings", None),
        ("controller_http_timings", Some("0")),
        ("controller_http_timings", Some("1")),
    ] {
        let mut command = tokio::process::Command::new(&executable);
        command
            .args([
                "--exact",
                &format!("controller::benchmarks::{test}"),
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .current_dir(root)
            .kill_on_drop(true)
            .env("RADIO_SEARCH_BENCH_DIR", root)
            .env("RADIO_SEARCH_BENCH_BINARY", &server)
            .env("RADIO_SEARCH_BENCH_WORKDIR", root);
        if let Some(compact) = compact {
            command.env("RADIO_SEARCH_BENCH_COMPACT", compact);
        }
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        let output = tokio::time::timeout(Duration::from_secs(30), command.output())
            .await
            .unwrap()
            .unwrap();
        assert!(
            output.status.success(),
            "{test} compact={compact:?}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        let timings = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            timings
                .lines()
                .filter(|line| line.starts_with(if compact.is_some() {
                    "q="
                } else {
                    "query_index="
                }))
                .count(),
            10
        );
    }
    let requests: Vec<(String, String)> = std::fs::read_to_string(root.join("requests.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(requests.len(), 120);
    for path in ["/api/beatmap-sets", "/api/tracks"] {
        for query in ["rock", "rock hard", "星", "does-not-exist-zzzz", ""] {
            assert_eq!(
                requests
                    .iter()
                    .filter(
                        |(actual_path, actual_query)| actual_path == path && actual_query == query
                    )
                    .count(),
                12
            );
        }
    }
}

#[test]
#[ignore = "requires RADIO_SEARCH_BENCH_DIR with baseline and stage3 HTTP payloads"]
fn captured_client_timings() {
    let directory = std::env::var("RADIO_SEARCH_BENCH_DIR").unwrap();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    for index in 0..5 {
        let original: Vec<BeatmapSet> = serde_json::from_slice(
            &std::fs::read(format!(
                "{directory}/baseline-{profile}-beatmap-sets-{index}.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let original_tracks = original_library_tracks(&original);
        for compact in [false, true] {
            let file = if compact {
                format!("stage3-{profile}-tracks-{index}.json")
            } else {
                format!("baseline-{profile}-beatmap-sets-{index}.json")
            };
            let bytes = std::fs::read(format!("{directory}/{file}")).unwrap();
            let mut results = Vec::new();
            for sample in 0..6 {
                let start = Instant::now();
                let (tracks, decode, conversion) = if compact {
                    let rows: Vec<crate::models::LibraryTrack> =
                        serde_json::from_slice(&bytes).unwrap();
                    let decode = start.elapsed();
                    let start = Instant::now();
                    let tracks: Vec<_> = rows.into_iter().map(Track::from).collect();
                    (tracks, decode, start.elapsed())
                } else {
                    let rows: Vec<BeatmapSet> = serde_json::from_slice(&bytes).unwrap();
                    let decode = start.elapsed();
                    let start = Instant::now();
                    let tracks = original_library_tracks(&rows);
                    (tracks, decode, start.elapsed())
                };
                if sample == 0 {
                    let mut presentation = tracks.clone();
                    for track in &mut presentation {
                        track.difficulties.clear();
                    }
                    assert_eq!(
                        presentation, original_tracks,
                        "captured query {index}, compact={compact}"
                    );
                }
                let mut controller = Controller::new(
                    ServerOptions::default(),
                    Arc::new(|event| {
                        std::hint::black_box(event);
                    }),
                );
                let start = Instant::now();
                controller.replace_tracks(tracks, true);
                let update = start.elapsed();
                if sample != 0 {
                    results.push((
                        decode.as_secs_f64() * 1000.,
                        conversion.as_secs_f64() * 1000.,
                        update.as_secs_f64() * 1000.,
                    ));
                }
            }
            let mut decode: Vec<_> = results.iter().map(|r| r.0).collect();
            let mut conversion: Vec<_> = results.iter().map(|r| r.1).collect();
            let mut update: Vec<_> = results.iter().map(|r| r.2).collect();
            decode.sort_by(f64::total_cmp);
            conversion.sort_by(f64::total_cmp);
            update.sort_by(f64::total_cmp);
            eprintln!(
                "query_index={index} compact={compact} decode_ms={:.3} conversion_ms={:.3} controller_ms={:.3}",
                decode[2], conversion[2], update[2]
            );
        }
    }
}

fn original_library_tracks(sets: &[BeatmapSet]) -> Vec<Track> {
    let mut order = Vec::new();
    let mut groups: HashMap<i32, Vec<(&BeatmapDetails, bool)>> = HashMap::new();
    for set in sets {
        for audio in &set.audio_sources {
            if !groups.contains_key(&audio.id) {
                order.push(audio.id);
            }
            groups.entry(audio.id).or_default();
        }
        let split = set.has_multiple_audio_sources || set.audio_sources.len() > 1;
        for map in &set.beatmaps {
            if let Some(id) = map.audio_source_id
                && let Some(maps) = groups.get_mut(&id)
            {
                maps.push((map, split));
            }
        }
    }
    order
        .into_iter()
        .map(|id| {
            let mut maps = groups.remove(&id).unwrap_or_default();
            maps.sort_by_key(|(map, _)| map.id);
            let representative = maps.first().map(|(map, _)| *map);
            let title = text(
                representative.and_then(|m| m.title.as_deref()),
                representative.and_then(|m| m.title_unicode.as_deref()),
                "Unknown title",
            );
            let artist = text(
                representative.and_then(|m| m.artist.as_deref()),
                representative.and_then(|m| m.artist_unicode.as_deref()),
                "Unknown artist",
            );
            let mut names = Vec::new();
            for (map, split) in &maps {
                if *split {
                    let name = text(map.difficulty_name.as_deref(), None, "Unknown difficulty");
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
            let subtitle = if names.is_empty() {
                artist.clone()
            } else {
                format!("{artist} | {}", names.join(", "))
            };
            Track {
                audio_source_id: id,
                last_played_at_ms: None,
                volume_percent: None,
                cover_beatmap_id: maps.iter().find(|(m, _)| m.has_cover).map(|(m, _)| m.id),
                title_original: representative.and_then(|map| map.title.clone()),
                title_unicode: representative.and_then(|map| map.title_unicode.clone()),
                artist_original: representative.and_then(|map| map.artist.clone()),
                artist_unicode: representative.and_then(|map| map.artist_unicode.clone()),
                subtitle_suffix: subtitle
                    .strip_prefix(&artist)
                    .unwrap_or_default()
                    .to_owned(),
                title,
                artist,
                subtitle,
                duration: None,
                difficulties: Vec::new(),
            }
        })
        .collect()
}

fn text(ordinary: Option<&str>, unicode: Option<&str>, unknown: &str) -> String {
    ordinary
        .filter(|s| !s.trim().is_empty())
        .or_else(|| unicode.filter(|s| !s.trim().is_empty()))
        .unwrap_or(unknown)
        .to_owned()
}

/// Actual HTTP -> decode -> mapping -> controller notification, with and without debounce.
/// Set the selected database URL explicitly to a disposable backup; Linux children skip dotenv.
#[tokio::test]
#[ignore = "requires RADIO_SEARCH_BENCH_BINARY, RADIO_SEARCH_BENCH_WORKDIR and RADIO_SEARCH_BENCH_COMPACT"]
async fn controller_http_timings() {
    let compact = std::env::var("RADIO_SEARCH_BENCH_COMPACT").unwrap() == "1";
    let options = ServerOptions {
        binary: Some(
            std::env::var_os("RADIO_SEARCH_BENCH_BINARY")
                .unwrap()
                .into(),
        ),
        working_directory: Some(
            std::env::var_os("RADIO_SEARCH_BENCH_WORKDIR")
                .unwrap()
                .into(),
        ),
        ..ServerOptions::default()
    };
    let session = Session::start(options.clone()).await.unwrap();
    let mut controller = Controller::new(
        options,
        Arc::new(|event| {
            std::hint::black_box(event);
        }),
    );
    controller.session = Some(session);
    for query in ["rock", "rock hard", "星", "does-not-exist-zzzz", ""] {
        for debounce in [false, true] {
            let delay = if debounce {
                Duration::from_millis(200)
            } else {
                Duration::ZERO
            };
            let mut samples = Vec::new();
            for sample in 0..6 {
                controller.library.query = query.into();
                let start = Instant::now();
                if compact {
                    controller.load_library(delay, false);
                    let completion = controller.tasks.join_next().await.unwrap().unwrap();
                    controller.complete(completion);
                } else {
                    // Original request/mapping path, reproduced for saved baseline binaries.
                    tokio::time::sleep(delay).await;
                    let sets = controller
                        .session
                        .as_ref()
                        .unwrap()
                        .api()
                        .search_beatmap_sets(query)
                        .await
                        .unwrap();
                    let tracks = original_library_tracks(&sets);
                    controller.complete(Completed::Library {
                        request: controller.library.request,
                        invalidate_artwork: true,
                        result: Ok(tracks),
                    });
                }
                assert!(controller.library.status.retry.is_none());
                if sample != 0 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "q={query:?} compact={compact} debounce={debounce} controller_http_ms={:.3}",
                samples[2]
            );
        }
    }
    controller.session.take().unwrap().shutdown().await.unwrap();
}
