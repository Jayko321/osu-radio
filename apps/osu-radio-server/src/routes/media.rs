use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use axum::{
    Json,
    extract::{Path as Id, State},
    http::header,
    response::IntoResponse,
};
use lofty::{config::ParseOptions, file::AudioFile, probe::Probe};
use serde::Serialize;

use crate::{error::ApiError, state::AppState};

const MAX_COVER_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct DurationResponse {
    duration_ms: Option<u64>,
}

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/beatmaps/{id}/cover",
    params(("id" = i32, Path)), responses((status = OK, content_type = "application/octet-stream", body = Vec<u8>),
    (status = NOT_FOUND, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn cover(
    State(state): State<AppState>,
    Id(id): Id<i32>,
) -> Result<impl IntoResponse, ApiError> {
    let path = state
        .services()
        .beatmaps()
        .get(id)
        .await?
        .and_then(|map| map.background_path)
        .ok_or_else(|| ApiError::not_found("Cover unavailable"))?;
    let bytes = tokio::task::spawn_blocking(move || read_cover(Path::new(&path)))
        .await?
        .ok_or_else(|| ApiError::not_found("Cover unavailable"))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    ))
}

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/audio-sources/{id}/duration",
    params(("id" = i32, Path)), responses((status = OK, body = DurationResponse),
    (status = NOT_FOUND, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn duration(
    State(state): State<AppState>,
    Id(id): Id<i32>,
) -> Result<Json<DurationResponse>, ApiError> {
    let audio = state
        .services()
        .audio_sources()
        .get(id)
        .await?
        .ok_or_else(|| ApiError::not_found("Audio source not found"))?;
    let duration_ms = match audio.s_type {
        radio_services::model::SourceType::Local(path)
        | radio_services::model::SourceType::Copied(path) => {
            tokio::task::spawn_blocking(move || probe_duration(Path::new(&path))).await?
        }
        radio_services::model::SourceType::Online(_) => None,
    };
    Ok(Json(DurationResponse { duration_ms }))
}

fn read_cover(path: &Path) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_COVER_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (u64::try_from(bytes.len()).ok()? <= MAX_COVER_BYTES).then_some(bytes)
}

fn probe_duration(path: &Path) -> Option<u64> {
    let file = File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let audio = Probe::new(BufReader::new(file))
        .guess_file_type()
        .ok()?
        .options(ParseOptions::new().read_tags(false))
        .read()
        .ok()?;
    let duration = audio.properties().duration();
    if duration.is_zero() {
        return None;
    }
    u64::try_from(duration.as_millis()).ok()
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::test_support::{beatmap_set, state_with};
    use axum::response::IntoResponse;
    use radio_core::import_types::{RealmFile, RealmNamedFileUsage};

    #[test]
    fn probes_extensionless_wav_mp3_and_ogg_and_rejects_missing_or_corrupt_media() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hash-without-extension");
        for bytes in [
            include_bytes!("../../tests/fixtures/tone.wav").as_slice(),
            include_bytes!("../../tests/fixtures/tone.mp3").as_slice(),
            include_bytes!("../../tests/fixtures/tone.ogg").as_slice(),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let duration = probe_duration(&path).unwrap();
            assert!((200..=350).contains(&duration), "duration: {duration}");
        }
        std::fs::write(&path, b"broken media").unwrap();
        assert_eq!(probe_duration(&path), None);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(probe_duration(&path), None);
        assert_eq!(read_cover(&path), None);
        assert_eq!(probe_duration(directory.path()), None);
    }

    #[tokio::test]
    async fn serves_only_stored_media_ids_and_handles_missing_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cover");
        let bytes = include_bytes!("../../../osu-radio-gui-vizia/assets/covers/karakara.jpg");
        std::fs::write(&path, bytes).unwrap();
        let mut set = beatmap_set(1, "/missing/audio");
        set.beatmaps[0].metadata.as_mut().unwrap().background_file = Some("bg.jpg".into());
        set.files.push(RealmNamedFileUsage {
            filename: Some("bg.jpg".into()),
            file: Some(RealmFile {
                hash: None,
                resolved_path: Some(path.clone()),
            }),
        });
        let state = state_with(&[set]).await;
        let sets = state
            .services()
            .beatmap_sets()
            .all_with_audio_sources()
            .await
            .unwrap();
        let map_id = sets[0].beatmaps[0].id;
        assert!(sets[0].beatmaps[0].has_cover);
        let response = cover(State(state.clone()), Id(map_id))
            .await
            .unwrap()
            .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            axum::body::to_bytes(
                response.into_body(),
                usize::try_from(MAX_COVER_BYTES).unwrap()
            )
            .await
            .unwrap()
            .as_ref(),
            bytes
        );
        let Json(duration) = duration(State(state.clone()), Id(sets[0].audio_sources[0].id))
            .await
            .unwrap();
        assert_eq!(duration.duration_ms, None);
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            cover(State(state.clone()), Id(map_id))
                .await
                .err()
                .unwrap()
                .into_response()
                .status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            cover(State(state), Id(i32::MAX))
                .await
                .err()
                .unwrap()
                .into_response()
                .status(),
            axum::http::StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn bounds_cover_reads() {
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(MAX_COVER_BYTES + 1).unwrap();
        assert!(read_cover(file.path()).is_none());
    }
}
