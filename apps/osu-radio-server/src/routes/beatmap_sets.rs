use axum::{
    Json,
    extract::{Query, State},
};
use radio_services::BeatmapSetWithAudio;
use radio_services::model::AudioSource;
use serde::{Deserialize, Serialize};

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct BeatmapSetResponse {
    pub(crate) has_multiple_audio_sources: bool,
    pub(crate) id: i32,
    pub(crate) online_id: Option<i32>,
    pub(crate) hash: Option<String>,
    pub(crate) beatmaps: Vec<BeatmapDetailsResponse>,
    pub(crate) audio_sources: Vec<AudioSourceResponse>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct AudioSourceResponse {
    pub(crate) id: i32,
    #[cfg_attr(feature = "docs", schema(value_type = String, example = "local"))]
    pub(crate) kind: &'static str,
    pub(crate) location: String,
}

impl From<BeatmapSetWithAudio> for BeatmapSetResponse {
    fn from(listed: BeatmapSetWithAudio) -> Self {
        Self {
            has_multiple_audio_sources: listed.has_multiple_audio_sources,
            beatmaps: listed
                .beatmaps
                .into_iter()
                .map(BeatmapDetailsResponse::from)
                .collect(),
            id: listed.beatmap_set.id,
            online_id: listed.beatmap_set.online_id,
            hash: listed.beatmap_set.hash,
            audio_sources: listed
                .audio_sources
                .into_iter()
                .map(AudioSourceResponse::from)
                .collect(),
        }
    }
}

impl From<AudioSource> for AudioSourceResponse {
    fn from(audio_source: AudioSource) -> Self {
        Self {
            id: audio_source.id,
            kind: audio_source.s_type.kind(),
            location: audio_source.s_type.location().to_owned(),
        }
    }
}

#[derive(Default, Deserialize)]
pub(crate) struct LibraryQuery {
    #[serde(default)]
    pub(super) q: String,
}

#[cfg_attr(
    feature = "docs",
    utoipa::path(
        get,
        path = "/api/beatmap-sets",
        tag = "beatmap-sets",
        description = "Every stored beatmap set with the audio sources its difficulties reference.",
        params(("q" = Option<String>, Query, description = "All whitespace-separated words must match literal case-insensitive substrings in tags, title, artist or difficulty (including Unicode variants); blank returns all.")),
        responses(
            (status = OK, body = Vec<BeatmapSetResponse>),
            (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody)
        )
    )
)]
pub(crate) async fn list_beatmap_sets(
    State(state): State<AppState>,
    Query(query): Query<LibraryQuery>,
) -> Result<Json<Vec<BeatmapSetResponse>>, ApiError> {
    let beatmap_sets = state
        .services()
        .beatmap_sets()
        .search_with_audio_sources(&query.q)
        .await?;

    Ok(Json(
        beatmap_sets
            .into_iter()
            .map(BeatmapSetResponse::from)
            .collect(),
    ))
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use crate::test_support::{beatmap_set, state_with};

    use super::*;

    #[tokio::test]
    async fn http_query_decodes_literal_symbols_and_returns_only_matching_audio() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut first = beatmap_set(1, "/search/first.mp3");
        for map in &mut first.beatmaps {
            map.metadata.as_mut().unwrap().tags = Some("ЁЖ a+b%_&?#".into());
        }
        // A different audio in the same set must not leak through a difficulty match.
        let mut separate = beatmap_set(2, "/search/second.mp3");
        separate.files[0].filename = Some("other.mp3".into());
        for map in &mut separate.beatmaps {
            map.difficulty_name = Some("Insane".into());
            map.metadata.as_mut().unwrap().audio_file = Some("other.mp3".into());
        }
        first.files.extend(separate.files);
        first.beatmaps.extend(separate.beatmaps);
        let state = state_with(&[first, beatmap_set(3, "/search/unrelated.mp3")]).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, crate::routes::router(state))
                .await
                .unwrap();
        });
        for (query, expected_sets) in [
            ("", 2),
            ("?q=", 2),
            ("?q=%D1%91%D0%B6+a%2Bb%25_%26%3F%23+hard", 1),
            ("?q=does-not-exist", 0),
        ] {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream.write_all(format!("GET /api/beatmap-sets{query} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).await.unwrap();
            assert!(response.starts_with("HTTP/1.1 200"), "{response}");
            let body: serde_json::Value =
                serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body.as_array().unwrap().len(), expected_sets);
            if expected_sets == 1 {
                assert_eq!(body[0]["has_multiple_audio_sources"], true);
                assert_eq!(body[0]["audio_sources"].as_array().unwrap().len(), 1);
                assert_eq!(body[0]["audio_sources"][0]["location"], "/search/first.mp3");
                assert_eq!(body[0]["beatmaps"].as_array().unwrap().len(), 2);
            }
        }
        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn responds_with_every_beatmap_set_and_its_audio_sources() {
        let state = state_with(&[beatmap_set(1, "/osu/files/a/ab/abc")]).await;

        let Json(listed) = list_beatmap_sets(State(state), Query(LibraryQuery::default()))
            .await
            .expect("beatmap sets should be listed");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].online_id, Some(1));
        assert_eq!(listed[0].hash.as_deref(), Some("set-hash-1"));
        assert_eq!(listed[0].audio_sources.len(), 1);
        assert_eq!(listed[0].beatmaps.len(), 2);
        assert_eq!(listed[0].beatmaps[0].title.as_deref(), Some("Song"));
        assert_eq!(listed[0].beatmaps[0].artist.as_deref(), Some("Artist"));
        assert_eq!(
            listed[0].beatmaps[0].difficulty_name.as_deref(),
            Some("Easy")
        );
        assert_eq!(
            listed[0].beatmaps[0].audio_source_id,
            Some(listed[0].audio_sources[0].id)
        );
        assert!(!listed[0].beatmaps[0].has_cover);
        assert_eq!(listed[0].audio_sources[0].kind, "local");
        assert_eq!(listed[0].audio_sources[0].location, "/osu/files/a/ab/abc");
    }

    #[tokio::test]
    async fn cloned_state_reads_replacements_and_installation_deletion() {
        let state = state_with(&[beatmap_set(1, "/osu/old.mp3")]).await;
        let cloned = state.clone();
        let installation = state
            .services()
            .osu_installations()
            .all()
            .await
            .expect("installations should load")
            .remove(0);
        state
            .services()
            .osu_installations()
            .replace_snapshot(installation.id, &[beatmap_set(2, "/osu/new.mp3")])
            .await
            .expect("replacement should store");
        let Json(listed) = list_beatmap_sets(State(cloned.clone()), Query(LibraryQuery::default()))
            .await
            .expect("replacement should be listed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].online_id, Some(2));
        assert_eq!(listed[0].audio_sources[0].location, "/osu/new.mp3");

        state
            .services()
            .osu_installations()
            .delete(installation.id)
            .await
            .expect("installation should delete");
        let Json(listed) = list_beatmap_sets(State(cloned), Query(LibraryQuery::default()))
            .await
            .expect("empty library should be listed");
        assert!(listed.is_empty());
    }

    #[tokio::test]
    async fn responds_with_an_empty_list_when_nothing_is_stored() {
        let state = state_with(&[]).await;

        let Json(listed) = list_beatmap_sets(State(state), Query(LibraryQuery::default()))
            .await
            .expect("beatmap sets should be listed");

        assert!(listed.is_empty());
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct BeatmapDetailsResponse {
    id: i32,
    audio_source_id: Option<i32>,
    difficulty_name: Option<String>,
    title: Option<String>,
    title_unicode: Option<String>,
    artist: Option<String>,
    artist_unicode: Option<String>,
    has_cover: bool,
}
impl From<radio_services::model::BeatmapDetails> for BeatmapDetailsResponse {
    fn from(map: radio_services::model::BeatmapDetails) -> Self {
        Self {
            id: map.id,
            audio_source_id: map.audio_source_id,
            difficulty_name: map.difficulty_name,
            title: map.title,
            title_unicode: map.title_unicode,
            artist: map.artist,
            artist_unicode: map.artist_unicode,
            has_cover: map.has_cover,
        }
    }
}
