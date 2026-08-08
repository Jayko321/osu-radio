#[path = "audio_sources.rs"]
mod audio_sources_schema;
#[path = "beatmap_metadata.rs"]
mod beatmap_metadata_schema;
#[path = "beatmap_sets.rs"]
mod beatmap_sets_schema;
#[path = "beatmaps.rs"]
mod beatmaps_schema;

pub use audio_sources_schema::audio_sources;
pub use beatmap_metadata_schema::beatmap_metadata;
pub use beatmap_sets_schema::beatmap_sets;
pub use beatmaps_schema::beatmaps;

diesel::joinable!(beatmap_metadata -> audio_sources (audio_source_id));
diesel::joinable!(beatmaps -> beatmap_metadata (metadata_id));
diesel::joinable!(beatmaps -> beatmap_sets (beatmap_set_id));

diesel::allow_tables_to_appear_in_same_query!(
    audio_sources,
    beatmap_metadata,
    beatmap_sets,
    beatmaps,
);
