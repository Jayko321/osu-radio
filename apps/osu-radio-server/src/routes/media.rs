use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use axum::{
    Json,
    body::Body,
    extract::{Path as Id, State},
    http::header,
    response::{IntoResponse, Response},
};
use lofty::{config::ParseOptions, file::AudioFile, probe::Probe};
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

use crate::{error::ApiError, state::AppState};

const MAX_COVER_BYTES: u64 = 16 * 1024 * 1024;
const AUDIO_CHUNK_BYTES: usize = 64 * 1024;

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/audio-sources/{id}/audio",
    params(("id" = i32, Path)), responses((status = OK, content_type = "application/octet-stream", body = Vec<u8>),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody),
    (status = NOT_FOUND, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn audio(
    State(state): State<AppState>,
    Id(id): Id<i32>,
) -> Result<Response, ApiError> {
    let source = state
        .services()
        .audio_sources()
        .get(id)
        .await?
        .ok_or_else(|| ApiError::not_found("Audio source not found"))?;
    let path = match source.s_type {
        radio_services::model::SourceType::Local(path)
        | radio_services::model::SourceType::Copied(path) => path,
        radio_services::model::SourceType::Online(_) => {
            return Err(ApiError::bad_request(
                "Online audio sources are not supported",
            ));
        }
    };
    // Check before opening: opening a nonregular file such as a FIFO can block.
    let metadata = tokio::fs::metadata(&path).await.map_err(audio_file_error)?;
    if !metadata.is_file() {
        return Err(ApiError::not_found("Audio file unavailable"));
    }
    let file = tokio::fs::File::open(path)
        .await
        .map_err(audio_file_error)?;
    let metadata = file.metadata().await.map_err(audio_file_error)?;
    if !metadata.is_file() {
        return Err(ApiError::not_found("Audio file unavailable"));
    }
    let length = metadata.len();
    // Never buffer the complete track. Limit the reader to the advertised length
    // even if another process appends to the underlying file during transfer.
    let stream = ReaderStream::with_capacity(file.take(length), AUDIO_CHUNK_BYTES);
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, length)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from_stream(stream))?)
}

fn audio_file_error(error: std::io::Error) -> ApiError {
    match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory => {
            ApiError::not_found("Audio file unavailable")
        }
        _ => error.into(),
    }
}

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
    use crate::test_support::{beatmap_set, empty_state, state_with};
    use axum::response::IntoResponse;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use radio_core::import_types::{RealmFile, RealmNamedFileUsage};
    use radio_services::model::SourceType;
    use tower::ServiceExt;

    async fn audio_request(state: AppState, id: i32) -> Response {
        crate::routes::router(state)
            .oneshot(
                Request::get(format!("/api/audio-sources/{id}/audio"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[cfg(feature = "docs")]
    #[tokio::test]
    async fn audio_route_is_in_the_served_openapi_document() {
        let response = crate::routes::router(empty_state().await)
            .oneshot(Request::get("/docs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let html = std::str::from_utf8(&body).unwrap();
        let spec = html
            .split_once("type=\"application/json\">")
            .unwrap()
            .1
            .split_once("</script>")
            .unwrap()
            .0;
        let document: serde_json::Value = serde_json::from_str(spec).unwrap();
        let responses = &document["paths"]["/api/audio-sources/{id}/audio"]["get"]["responses"];
        assert!(responses["200"]["content"]["application/octet-stream"].is_object());
        for status in ["400", "404", "500"] {
            assert!(responses[status].is_object());
        }
    }

    #[tokio::test]
    async fn audio_http_streams_exact_bytes_in_bounded_chunks_for_local_and_copied_sources() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("extensionless-audio");
        let state = empty_state().await;
        for bytes in [
            include_bytes!("../../tests/fixtures/tone.mp3").to_vec(),
            (0_u8..=255).cycle().take(3 * 1024 * 1024 + 17).collect(),
            Vec::new(),
        ] {
            std::fs::write(&path, &bytes).unwrap();
            for source in [
                SourceType::Local(path.to_str().unwrap().into()),
                SourceType::Copied(path.to_str().unwrap().into()),
            ] {
                let source = state
                    .services()
                    .audio_sources()
                    .get_or_insert(&source)
                    .await
                    .unwrap();
                let response = audio_request(state.clone(), source.id).await;
                assert_eq!(response.status(), axum::http::StatusCode::OK);
                assert_eq!(
                    response.headers()[header::CONTENT_TYPE],
                    "application/octet-stream"
                );
                assert_eq!(
                    response.headers()[header::CONTENT_LENGTH],
                    bytes.len().to_string()
                );
                let mut body = response.into_body();
                let mut received = Vec::new();
                let mut frames = 0_usize;
                while let Some(frame) = body.frame().await {
                    let chunk = frame.unwrap().into_data().unwrap();
                    assert!(chunk.len() <= AUDIO_CHUNK_BYTES);
                    frames = frames.saturating_add(1);
                    received.extend_from_slice(&chunk);
                }
                assert_eq!(received, bytes);
                if bytes.len() > AUDIO_CHUNK_BYTES {
                    assert!(frames > 1);
                }
            }
        }
    }

    #[tokio::test]
    async fn audio_http_rejects_unknown_missing_nonregular_and_online_sources() {
        let directory = tempfile::tempdir().unwrap();
        let state = empty_state().await;
        let missing = directory.path().join("missing");
        let regular = directory.path().join("file");
        std::fs::write(&regular, b"audio").unwrap();
        let not_a_directory = regular.join("child");
        for (source, expected) in [
            (
                SourceType::Local(missing.to_str().unwrap().into()),
                axum::http::StatusCode::NOT_FOUND,
            ),
            (
                SourceType::Copied(missing.to_str().unwrap().into()),
                axum::http::StatusCode::NOT_FOUND,
            ),
            (
                SourceType::Local(directory.path().to_str().unwrap().into()),
                axum::http::StatusCode::NOT_FOUND,
            ),
            (
                SourceType::Local(not_a_directory.to_str().unwrap().into()),
                axum::http::StatusCode::NOT_FOUND,
            ),
            (
                SourceType::Online("https://example.invalid/audio.mp3".into()),
                axum::http::StatusCode::BAD_REQUEST,
            ),
        ] {
            let source = state
                .services()
                .audio_sources()
                .get_or_insert(&source)
                .await
                .unwrap();
            let response = audio_request(state.clone(), source.id).await;
            assert_eq!(response.status(), expected);
            let bytes = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert!(json["error"].is_string());
            assert!(
                !std::str::from_utf8(&bytes)
                    .unwrap()
                    .contains(directory.path().to_str().unwrap())
            );
        }
        assert_eq!(
            audio_request(state.clone(), i32::MAX).await.status(),
            axum::http::StatusCode::NOT_FOUND
        );
        let source = state
            .services()
            .audio_sources()
            .get_or_insert(&SourceType::Local(regular.to_str().unwrap().into()))
            .await
            .unwrap();
        std::fs::remove_file(regular).unwrap();
        assert_eq!(
            audio_request(state, source.id).await.status(),
            axum::http::StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn audio_body_reads_lazily_and_caps_growth_at_advertised_length() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("changing-file");
        std::fs::write(&path, b"old").unwrap();
        let state = empty_state().await;
        let source = state
            .services()
            .audio_sources()
            .get_or_insert(&SourceType::Local(path.to_str().unwrap().into()))
            .await
            .unwrap();
        let response = audio_request(state, source.id).await;
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "3");
        std::fs::write(&path, b"new and appended bytes").unwrap();
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap()
                .as_ref(),
            b"new"
        );
    }

    #[tokio::test]
    async fn client_download_roundtrip_preserves_bytes_and_removes_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source-without-extension");
        let bytes: Vec<u8> = (0_u8..=255).cycle().take(2 * 1024 * 1024 + 7).collect();
        std::fs::write(&path, &bytes).unwrap();
        let state = empty_state().await;
        let source = state
            .services()
            .audio_sources()
            .get_or_insert(&SourceType::Local(path.to_str().unwrap().into()))
            .await
            .unwrap();
        let online = state
            .services()
            .audio_sources()
            .get_or_insert(&SourceType::Online("https://example.invalid/song".into()))
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api =
            osu_radio_client::ApiClient::new(format!("http://{}", listener.local_addr().unwrap()))
                .unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, crate::routes::router(state))
                .await
                .unwrap();
        });
        let temporary = api.download_audio(source.id).await.unwrap();
        let temporary_path = temporary.path().to_owned();
        assert_eq!(std::fs::read(&temporary_path).unwrap(), bytes);
        drop(temporary);
        assert!(!temporary_path.exists());
        assert!(path.exists());
        assert!(
            api.download_audio(online.id)
                .await
                .unwrap_err()
                .to_string()
                .contains("Online audio sources are not supported")
        );
        assert!(
            api.download_audio(i32::MAX)
                .await
                .unwrap_err()
                .to_string()
                .contains("Audio source not found")
        );
        server.abort();
        let _ = server.await;
    }

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
