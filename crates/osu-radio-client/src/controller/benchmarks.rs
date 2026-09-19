//! Opt-in measurements of captured HTTP payloads, without GUI rendering or media I/O.
#![allow(clippy::indexing_slicing)]
use super::*;
use crate::{BeatmapSet, models::BeatmapDetails};
use std::time::Instant;

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
                controller.replace_tracks(tracks);
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
                cover_beatmap_id: maps.iter().find(|(m, _)| m.has_cover).map(|(m, _)| m.id),
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
/// The child must be configured to use a disposable backup by its working directory.
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
                controller.library_query = query.into();
                let start = Instant::now();
                if compact {
                    controller.load_library(delay);
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
                        request: controller.library_request,
                        result: Ok(tracks),
                    });
                }
                assert!(controller.library.retry.is_none());
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
