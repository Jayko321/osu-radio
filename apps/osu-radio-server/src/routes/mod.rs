pub(crate) mod audio_settings;
pub(crate) mod beatmap_sets;
pub(crate) mod folder_selection;
pub(crate) mod media;
pub(crate) mod playback;
pub(crate) mod playlists;
pub(crate) mod tracks;
pub(crate) mod user_data;

use axum::Router;

use crate::state::AppState;

/// Kept in lockstep with the `docs` variant below: a route added to one belongs in both.
#[cfg(not(feature = "docs"))]
pub(crate) fn router(state: AppState) -> Router {
    use axum::routing::{get, patch, post};

    Router::new()
        .route("/api/beatmaps/{id}/cover", get(media::cover))
        .route("/api/audio-sources/{id}/duration", get(media::duration))
        .route("/api/audio-sources/{id}/audio", get(media::audio))
        .route("/api/tracks", get(tracks::list_tracks))
        .route(
            "/api/playlists",
            get(playlists::list).post(playlists::create),
        )
        .route(
            "/api/playlists/{id}",
            get(playlists::get)
                .patch(playlists::rename)
                .delete(playlists::delete),
        )
        .route("/api/playlists/{id}/items", post(playlists::add_items))
        .route(
            "/api/playlists/{id}/items/{item_id}",
            axum::routing::delete(playlists::remove_item),
        )
        .route("/api/playlists/{id}/play", post(playlists::play))
        .route(
            "/api/playlists/{id}/cover",
            get(playlists::get_cover)
                .put(playlists::put_cover)
                .delete(playlists::delete_cover)
                .layer(axum::extract::DefaultBodyLimit::max(
                    radio_services::MAX_PLAYLIST_COVER_BYTES,
                )),
        )
        .route(
            "/api/queue",
            get(playback::get_queue).delete(playback::clear_queue),
        )
        .route("/api/queue/items", post(playback::append_items))
        .route("/api/playback", get(playback::get_playback))
        .route("/api/playback/events", get(playback::events))
        .route("/api/playback/commands", post(playback::command))
        .route("/api/beatmap-sets", get(beatmap_sets::list_beatmap_sets))
        .route("/api/user-data", get(user_data::get_user_data))
        .route(
            "/api/user-data/audio-settings",
            get(audio_settings::get).patch(audio_settings::patch),
        )
        .route(
            "/api/audio-sources/{id}/volume",
            axum::routing::put(audio_settings::put_volume).delete(audio_settings::delete_volume),
        )
        .route(
            "/api/user-data/osu-folders/discover",
            post(folder_selection::discover),
        )
        .route(
            "/api/user-data/osu-folders/metadata",
            post(folder_selection::metadata),
        )
        .route(
            "/api/user-data/osu-folders/import",
            post(folder_selection::import),
        )
        .route(
            "/api/user-data/osu-folders/{id}/import",
            post(folder_selection::reimport),
        )
        .route(
            "/api/user-data/osu-folders",
            get(user_data::list_osu_folders).post(user_data::register_osu_folder),
        )
        .route(
            "/api/user-data/osu-folders/{id}",
            patch(user_data::update_osu_folder).delete(user_data::remove_osu_folder),
        )
        .with_state(state)
}

#[cfg(feature = "docs")]
pub(crate) fn router(state: AppState) -> Router {
    use utoipa::OpenApi;
    use utoipa_axum::{router::OpenApiRouter, routes};
    use utoipa_scalar::{Scalar, Servable};

    use crate::docs::{ApiDoc, SCALAR_PATH};

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(beatmap_sets::list_beatmap_sets))
        .routes(routes!(tracks::list_tracks))
        .routes(routes!(playlists::list, playlists::create))
        .routes(routes!(
            playlists::get,
            playlists::rename,
            playlists::delete
        ))
        .routes(routes!(playlists::add_items))
        .routes(routes!(playlists::remove_item))
        .routes(routes!(playlists::play))
        .routes(routes!(
            playlists::get_cover,
            playlists::put_cover,
            playlists::delete_cover
        ))
        .routes(routes!(playback::get_queue, playback::clear_queue))
        .routes(routes!(playback::append_items))
        .routes(routes!(playback::get_playback))
        .routes(routes!(playback::events))
        .routes(routes!(playback::command))
        .routes(routes!(media::cover))
        .routes(routes!(media::duration))
        .routes(routes!(media::audio))
        .routes(routes!(user_data::get_user_data))
        .routes(routes!(audio_settings::get, audio_settings::patch))
        .routes(routes!(
            audio_settings::put_volume,
            audio_settings::delete_volume
        ))
        .routes(routes!(folder_selection::discover))
        .routes(routes!(folder_selection::metadata))
        .routes(routes!(folder_selection::import))
        .routes(routes!(folder_selection::reimport))
        .routes(routes!(
            user_data::list_osu_folders,
            user_data::register_osu_folder
        ))
        .routes(routes!(
            user_data::update_osu_folder,
            user_data::remove_osu_folder
        ))
        .with_state(state)
        .split_for_parts();

    router
        .merge(Scalar::with_url(SCALAR_PATH, api))
        .layer(axum::extract::DefaultBodyLimit::max(
            radio_services::MAX_PLAYLIST_COVER_BYTES,
        ))
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    const API_ROUTES: &[(&str, &[&str])] = &[
        ("/api/beatmap-sets", &["get"]),
        ("/api/tracks", &["get"]),
        ("/api/beatmaps/{id}/cover", &["get"]),
        ("/api/audio-sources/{id}/duration", &["get"]),
        ("/api/audio-sources/{id}/audio", &["get"]),
        ("/api/audio-sources/{id}/volume", &["put", "delete"]),
        ("/api/playlists", &["get", "post"]),
        ("/api/playlists/{id}", &["get", "patch", "delete"]),
        ("/api/playlists/{id}/items", &["post"]),
        ("/api/playlists/{id}/items/{item_id}", &["delete"]),
        ("/api/playlists/{id}/play", &["post"]),
        ("/api/playlists/{id}/cover", &["get", "put", "delete"]),
        ("/api/queue", &["get", "delete"]),
        ("/api/queue/items", &["post"]),
        ("/api/playback", &["get"]),
        ("/api/playback/events", &["get"]),
        ("/api/playback/commands", &["post"]),
        ("/api/user-data", &["get"]),
        ("/api/user-data/audio-settings", &["get", "patch"]),
        ("/api/user-data/osu-folders", &["get", "post"]),
        ("/api/user-data/osu-folders/{id}", &["patch", "delete"]),
        ("/api/user-data/osu-folders/discover", &["post"]),
        ("/api/user-data/osu-folders/metadata", &["post"]),
        ("/api/user-data/osu-folders/import", &["post"]),
        ("/api/user-data/osu-folders/{id}/import", &["post"]),
    ];

    #[tokio::test]
    async fn every_api_path_and_method_is_routed_with_docs_enabled_or_disabled() {
        let state = crate::test_support::empty_state().await;
        let app = router(state.clone());
        for (path, methods) in API_ROUTES {
            let uri = path.replace("{id}", "7").replace("{item_id}", "8");
            for method in ["get", "post", "put", "patch", "delete", "options", "trace"] {
                // Malformed JSON rejects filesystem/discovery actions before their handlers run.
                let response = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method(method.to_ascii_uppercase().as_str())
                            .uri(&uri)
                            .header("content-type", "application/json")
                            .body(Body::from("{"))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                if methods.contains(&method) {
                    assert_ne!(
                        response.status(),
                        StatusCode::METHOD_NOT_ALLOWED,
                        "{method} {path}"
                    );
                    assert!(
                        !response.status().is_server_error(),
                        "{method} {path}: {}",
                        response.status()
                    );
                } else {
                    assert_eq!(
                        response.status(),
                        StatusCode::METHOD_NOT_ALLOWED,
                        "unregistered path or unexpected method: {method} {path}"
                    );
                }
            }
        }
        assert_eq!(
            state.playback_subscriber_count(),
            0,
            "dropping the event response releases its receiver"
        );
        let response = app
            .oneshot(Request::get("/docs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if cfg!(feature = "docs") {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            }
        );
    }

    #[cfg(feature = "docs")]
    #[tokio::test]
    async fn served_openapi_has_exact_api_methods_metadata_and_resolvable_schema_references() {
        use serde_json::Value;
        use std::collections::BTreeSet;

        fn check_references(value: &Value, root: &Value) {
            match value {
                Value::Object(object) => {
                    for (key, value) in object {
                        if key == "$ref" {
                            let reference = value.as_str().unwrap();
                            assert!(
                                root.pointer(reference.strip_prefix('#').unwrap()).is_some(),
                                "unresolved schema: {reference}"
                            );
                        } else {
                            check_references(value, root);
                        }
                    }
                }
                Value::Array(values) => {
                    for value in values {
                        check_references(value, root);
                    }
                }
                _ => {}
            }
        }
        let response = router(crate::test_support::empty_state().await)
            .oneshot(
                Request::get(crate::docs::SCALAR_PATH)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let html = std::str::from_utf8(&body).unwrap();
        let json = html
            .split_once("type=\"application/json\">")
            .unwrap()
            .1
            .split_once("</script>")
            .unwrap()
            .0;
        let document: Value = serde_json::from_str(json).unwrap();
        assert_eq!(document["info"]["title"], "osu-radio");
        assert_eq!(document["info"]["version"], env!("CARGO_PKG_VERSION"));
        assert!(!document["info"]["description"].as_str().unwrap().is_empty());
        let expected: BTreeSet<_> = API_ROUTES.iter().map(|(path, _)| *path).collect();
        let actual: BTreeSet<_> = document["paths"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(actual, expected);
        for (path, methods) in API_ROUTES {
            let actual: BTreeSet<_> = document["paths"][path]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(actual, methods.iter().copied().collect(), "{path}");
        }
        let tags: BTreeSet<_> = document["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tag| tag["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            tags,
            BTreeSet::from([
                "tracks",
                "playback",
                "playlists",
                "beatmap-sets",
                "user-data"
            ])
        );
        check_references(&document, &document);
    }
}
