// Frozen metadata entity used only while migrating the v1 schema.
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "beatmap_metadata")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub hash: String,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub author: Option<serde_json::Value>,
    pub source: Option<String>,
    pub tags: Option<String>,
    pub user_tags: serde_json::Value,
    pub preview_time: Option<i32>,
    pub audio_file: Option<String>,
    pub background_file: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
