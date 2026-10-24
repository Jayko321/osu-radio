use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "playback_queue")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub current_entry_id: Option<i64>,
    pub history_len: i64,
    pub last_entry_id: i64,
    pub mode: String,
    pub waiting_for_check: bool,
    pub launch_audio_source_id: Option<i32>,
    pub launch_audio_kind: Option<String>,
    pub launch_audio_location: Option<String>,
    pub launch_check_generation: Option<i64>,
    pub shuffle: bool,
    pub repeat_mode: String,
    pub repeat_launch: bool,
    pub revision: i64,
    pub playback_token: i64,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
