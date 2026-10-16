use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "queue_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub position: i64,
    pub audio_kind: Option<String>,
    pub audio_location: Option<String>,
    pub installation_kind: Option<String>,
    pub hash_kind: Option<String>,
    pub beatmap_hash: Option<String>,
    pub audio_source_id: Option<i32>,
    pub playlist_item_id: Option<i32>,
    pub migration_audio_source_id: Option<i32>,
    pub migration_playlist_item_id: Option<i32>,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub difficulty_name: Option<String>,
    pub beatmap_id: Option<i32>,
    pub beatmap_set_id: Option<i32>,
    pub cover_beatmap_id: Option<i32>,
    pub availability: String,
    pub check_generation: i64,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
