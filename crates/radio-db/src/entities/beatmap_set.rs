use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "beatmap_sets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub installation_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
