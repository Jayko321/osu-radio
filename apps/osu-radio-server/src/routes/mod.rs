pub(crate) mod beatmap_sets;
pub(crate) mod user_data;

use axum::Router;

use crate::state::AppState;

/// Kept in lockstep with the `docs` variant below: a route added to one belongs in both.
#[cfg(not(feature = "docs"))]
pub(crate) fn router(state: AppState) -> Router {
    use axum::routing::{get, patch};

    Router::new()
        .route("/api/beatmap-sets", get(beatmap_sets::list_beatmap_sets))
        .route("/api/user-data", get(user_data::get_user_data))
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
        .routes(routes!(user_data::get_user_data))
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
