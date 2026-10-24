use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "audio_mods")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub audio_kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_location: String,
    pub mode: String,
    pub speed_percent: i32,
    pub adjust_pitch: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
