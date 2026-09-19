use utoipa::OpenApi;

pub(crate) const SCALAR_PATH: &str = "/docs";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "osu-radio",
        description = "Backend over the imported osu! library: stored beatmap sets, their audio sources, and the registered osu! folders they came from."
    ),
    tags(
        (name = "tracks", description = "Songs grouped by global audio identity, with all difficulties."),
        (name = "beatmap-sets", description = "Beatmap sets persisted from an osu! installation."),
        (name = "user-data", description = "Settings and the osu! folders registered for scanning.")
    )
)]
pub(crate) struct ApiDoc;
