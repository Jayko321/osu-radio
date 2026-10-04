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
