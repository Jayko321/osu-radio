use diesel::prelude::{Identifiable, Insertable, Queryable, Selectable};

use crate::{model::AudioSource, schema::beatmap_metadata};

#[derive(Debug, Clone, Identifiable, Queryable, Selectable)]
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
    #[diesel(embed)]
    pub audio_source: Option<AudioSource>,
    pub background_file: Option<String>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = beatmap_metadata)]
pub struct NewBeatmapMetadata<'a> {
    pub title: Option<&'a str>,
    pub title_unicode: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub artist_unicode: Option<&'a str>,
    pub author_online_id: Option<i32>,
    pub author_username: Option<&'a str>,
    pub author_country_code: Option<&'a str>,
    pub source: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub user_tags: Option<String>,
    pub preview_time: Option<i32>,
    pub audio_source_id: Option<i32>,
    pub background_file: Option<&'a str>,
}
