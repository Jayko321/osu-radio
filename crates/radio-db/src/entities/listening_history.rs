use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "listening_history")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub audio_kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_location: String,
    pub last_played_at_ms: i64,
    pub playback_token: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
