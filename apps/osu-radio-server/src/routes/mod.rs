pub(crate) mod beatmap_sets;
pub(crate) mod folder_selection;
pub(crate) mod media;
pub(crate) mod playback;
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
        .routes(routes!(playback::get_queue, playback::clear_queue))
        .routes(routes!(playback::append_items))
        .routes(routes!(playback::get_playback))
        .routes(routes!(playback::events))
        .routes(routes!(playback::command))
        .routes(routes!(media::cover))
        .routes(routes!(media::duration))
        .routes(routes!(media::audio))
        .routes(routes!(user_data::get_user_data))
        .routes(routes!(folder_selection::discover))
        .routes(routes!(folder_selection::metadata))
        .routes(routes!(folder_selection::import))
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

    router.merge(Scalar::with_url(SCALAR_PATH, api))
}
