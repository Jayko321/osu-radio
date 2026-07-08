use crate::schema::{beatmap_metadata, beatmap_sets, beatmaps};

#[derive(Debug, Clone, diesel::Identifiable, diesel::Queryable)]
#[diesel(table_name = beatmaps)]
pub struct Beatmap {
    pub id: i32,
    pub source: String,
    pub difficulty_name: Option<String>,
    pub bpm: Option<f64>,
    pub hash: Option<String>,
    pub beatmap_set_id: Option<i32>,
    pub metadata_id: Option<i32>,
}

#[derive(Debug, Clone, diesel::Identifiable, diesel::Queryable)]
#[diesel(table_name = beatmap_sets)]
pub struct BeatmapSet {
    pub id: i32,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, diesel::Identifiable, diesel::Queryable)]
#[diesel(table_name = beatmap_metadata)]
pub struct BeatmapMetadata {
    pub id: i32,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub author_online_id: Option<i32>,
    pub author_username: Option<String>,
    pub author_country_code: Option<String>,
    pub source: Option<String>,
    pub tags: Option<String>,
    pub user_tags: Option<String>,
    pub preview_time: Option<i32>,
    pub audio_file: Option<String>,
    pub background_file: Option<String>,
}
