use super::beatmap_sets::LibraryQuery;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct TrackResponse {
    pub audio_source_id: i32,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub cover_beatmap_id: Option<i32>,
    pub difficulties: Vec<TrackDifficulty>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct TrackDifficulty {
    pub beatmap_id: i32,
    pub beatmap_set_id: i32,
    pub difficulty_name: Option<String>,
    pub set_has_multiple_audio_sources: bool,
}

impl From<radio_services::LibraryTrack> for TrackResponse {
    fn from(track: radio_services::LibraryTrack) -> Self {
        Self {
            audio_source_id: track.audio_source_id,
            title: track.title,
            title_unicode: track.title_unicode,
            artist: track.artist,
            artist_unicode: track.artist_unicode,
            cover_beatmap_id: track.cover_beatmap_id,
            difficulties: track
                .difficulties
                .into_iter()
                .map(|map| TrackDifficulty {
                    beatmap_id: map.beatmap_id,
                    beatmap_set_id: map.beatmap_set_id,
                    difficulty_name: map.difficulty_name,
                    set_has_multiple_audio_sources: map.set_has_multiple_audio_sources,
                })
                .collect(),
        }
    }
}

#[cfg_attr(feature = "docs", utoipa::path(
    get, path = "/api/tracks", tag = "tracks",
    description = "One row per global audio source, in library order, with every related difficulty. Metadata comes from the lowest beatmap ID; cover from the lowest ID with a stored reference.",
    params(("q" = Option<String>, Query, description = "Literal case-insensitive words matched across tags, metadata and difficulties of the same audio; blank returns all.")),
    responses((status = OK, body = Vec<TrackResponse>), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))
))]
pub(crate) async fn list_tracks(
    State(state): State<AppState>,
    Query(query): Query<LibraryQuery>,
) -> Result<Json<Vec<TrackResponse>>, ApiError> {
    Ok(Json(
        state
            .services()
            .beatmap_sets()
            .search_tracks(&query.q)
            .await?
            .into_iter()
            .map(TrackResponse::from)
            .collect(),
    ))
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::test_support::{beatmap_set, state_with};
    use osu_radio_client::{ApiClient, Track, view_models::library_tracks};

    #[tokio::test]
    async fn compact_http_preserves_legacy_tracks_and_every_difficulty() {
        let mut first = beatmap_set(1, "/shared.mp3");
        let metadata = first.beatmaps[0].metadata.as_mut().unwrap();
        metadata.title = Some(" ".into());
        metadata.title_unicode = Some("曲".into());
        metadata.artist = None;
        metadata.tags = Some("rock a+b%_&?#".into());
        first.beatmaps[0].difficulty_name = None;
        first.beatmaps[1].metadata.as_mut().unwrap().background_file = Some("cover.jpg".into());
        let mut file = first.files[0].clone();
        file.filename = Some("cover.jpg".into());
        file.file.as_mut().unwrap().resolved_path = Some("/missing/cover.jpg".into());
        first.files.push(file);
        let mut separate = beatmap_set(2, "/other.mp3");
        separate.files[0].filename = Some("other.mp3".into());
        for map in &mut separate.beatmaps {
            map.metadata.as_mut().unwrap().audio_file = Some("other.mp3".into());
        }
        first.files.extend(separate.files);
        first.beatmaps.extend(separate.beatmaps);
        let state = state_with(&[first, beatmap_set(3, "/shared.mp3")]).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, crate::routes::router(state))
                .await
                .unwrap();
        });
        for query in ["", "rock", "曲", "a+b%_&?# hard", "missing"] {
            let old = api.search_beatmap_sets(query).await.unwrap();
            let new = api.search_tracks(query).await.unwrap();
            let tracks: Vec<_> = new.into_iter().map(Track::from).collect();
            assert_eq!(tracks, library_tracks(&old), "{query}");
            if query.is_empty() {
                assert_eq!(tracks.len(), 2);
                assert_eq!(tracks[0].difficulties.len(), 4);
                assert_eq!(tracks[0].title, "曲");
                assert_eq!(tracks[0].artist, "Unknown artist");
                assert!(tracks[0].cover_beatmap_id.is_some());
                assert_eq!(
                    tracks[0].subtitle,
                    "Unknown artist | Unknown difficulty, Hard"
                );
            }
        }
        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    #[ignore = "requires RADIO_SEARCH_BENCH_DB pointing to a consistent backup"]
    async fn snapshot_serialization_timings() {
        use std::time::Instant;
        let state = AppState::connect(&std::env::var("RADIO_SEARCH_BENCH_DB").unwrap())
            .await
            .unwrap();
        for query in ["rock", "rock hard", "星", "does-not-exist-zzzz", ""] {
            let mut results = Vec::new();
            for sample in 0..6 {
                let sets = state
                    .services()
                    .beatmap_sets()
                    .search_with_audio_sources(query)
                    .await
                    .unwrap();
                let start = Instant::now();
                let old: Vec<_> = sets
                    .into_iter()
                    .map(super::super::beatmap_sets::BeatmapSetResponse::from)
                    .collect();
                let old_json = serde_json::to_vec(&old).unwrap();
                let old_time = start.elapsed();
                let start = Instant::now();
                let Json(new) = list_tracks(
                    State(state.clone()),
                    Query(LibraryQuery { q: query.into() }),
                )
                .await
                .unwrap();
                let service_and_dto = start.elapsed();
                let start = Instant::now();
                let new_json = serde_json::to_vec(&new).unwrap();
                let new_time = start.elapsed();
                let old: Vec<osu_radio_client::BeatmapSet> =
                    serde_json::from_slice(&old_json).unwrap();
                let new: Vec<osu_radio_client::models::LibraryTrack> =
                    serde_json::from_slice(&new_json).unwrap();
                assert_eq!(
                    library_tracks(&old),
                    new.into_iter().map(Track::from).collect::<Vec<_>>()
                );
                if sample != 0 {
                    results.push((
                        old_time.as_secs_f64() * 1000.,
                        new_time.as_secs_f64() * 1000.,
                        service_and_dto.as_secs_f64() * 1000.,
                    ));
                }
            }
            let mut old: Vec<_> = results.iter().map(|r| r.0).collect();
            let mut new: Vec<_> = results.iter().map(|r| r.1).collect();
            let mut service: Vec<_> = results.iter().map(|r| r.2).collect();
            old.sort_by(f64::total_cmp);
            new.sort_by(f64::total_cmp);
            service.sort_by(f64::total_cmp);
            eprintln!(
                "q={query:?} old_dto_serialize_ms={:.3} new_serialize_ms={:.3} tracks_service_dto_ms={:.3}",
                old[2], new[2], service[2]
            );
        }
    }
}
