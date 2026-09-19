use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "beatmap_set_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub beatmap_set_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tag_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
