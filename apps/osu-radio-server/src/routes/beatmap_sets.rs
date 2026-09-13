use axum::{Json, extract::State};
use radio_services::BeatmapSetWithAudio;
use radio_services::model::AudioSource;
use serde::Serialize;

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct BeatmapSetResponse {
    pub(crate) id: i32,
    pub(crate) online_id: Option<i32>,
    pub(crate) hash: Option<String>,
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

#[cfg_attr(
    feature = "docs",
    utoipa::path(
        get,
        path = "/api/beatmap-sets",
        tag = "beatmap-sets",
        description = "Every stored beatmap set with the audio sources its difficulties reference.",
        responses(
            (status = OK, body = Vec<BeatmapSetResponse>),
            (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody)
        )
    )
)]
pub(crate) async fn list_beatmap_sets(
    State(state): State<AppState>,
) -> Result<Json<Vec<BeatmapSetResponse>>, ApiError> {
    let beatmap_sets = state
        .services()
        .beatmap_sets()
        .all_with_audio_sources()
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
    async fn responds_with_every_beatmap_set_and_its_audio_sources() {
        let state = state_with(&[beatmap_set(1, "/osu/files/a/ab/abc")]).await;

        let Json(listed) = list_beatmap_sets(State(state))
            .await
            .expect("beatmap sets should be listed");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].online_id, Some(1));
        assert_eq!(listed[0].hash.as_deref(), Some("set-hash-1"));
        assert_eq!(listed[0].audio_sources.len(), 1);
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
        let Json(listed) = list_beatmap_sets(State(cloned.clone()))
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
        let Json(listed) = list_beatmap_sets(State(cloned))
            .await
            .expect("empty library should be listed");
        assert!(listed.is_empty());
    }

    #[tokio::test]
    async fn responds_with_an_empty_list_when_nothing_is_stored() {
        let state = state_with(&[]).await;

        let Json(listed) = list_beatmap_sets(State(state))
            .await
            .expect("beatmap sets should be listed");

        assert!(listed.is_empty());
    }
}
