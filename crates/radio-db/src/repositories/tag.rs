use crate::{
    entities::{beatmap_set_tag, tag},
    model::Tag,
};
use anyhow::{Context, Result};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set,
    sea_query::{Expr, ExprTrait, OnConflict, Query},
};

pub struct TagRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl TagRepository<'_> {
    /// Literal substring in already normalized tags. Resolve matching tag IDs first,
    /// then use the `tag_id` link index; never interpret user text as a SQL pattern.
    pub async fn matching_set_ids(&self, word: &str) -> Result<Vec<i32>> {
        #[cfg(feature = "sqlite")]
        let predicate = "instr(name, ?) > 0";
        #[cfg(feature = "postgres")]
        let predicate = "strpos(name, $1) > 0";
        let statement = sea_orm::Statement::from_sql_and_values(
            self.connection.get_database_backend(),
            format!(
                "SELECT DISTINCT beatmap_set_id FROM beatmap_set_tags WHERE tag_id IN (SELECT id FROM tags WHERE {predicate})"
            ),
            [word.into()],
        );
        Ok(SetIdRow::find_by_statement(statement)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(|row| row.beatmap_set_id)
            .collect())
    }

    /// Bulk set-tag relationships for a library snapshot, without per-set queries.
    pub async fn all_set_links(&self) -> Result<Vec<crate::model::SetTag>> {
        let query = Query::select()
            .column((
                beatmap_set_tag::Entity,
                beatmap_set_tag::Column::BeatmapSetId,
            ))
            .column((tag::Entity, tag::Column::Name))
            .from(beatmap_set_tag::Entity)
            .inner_join(
                tag::Entity,
                Expr::col((beatmap_set_tag::Entity, beatmap_set_tag::Column::TagId))
                    .equals((tag::Entity, tag::Column::Id)),
            )
            .to_owned();
        Ok(
            SetTagRow::find_by_statement(self.connection.get_database_backend().build(&query))
                .all(&self.connection)
                .await?
                .into_iter()
                .map(|row| crate::model::SetTag {
                    beatmap_set_id: row.beatmap_set_id,
                    name: row.name,
                })
                .collect(),
        )
    }

    pub async fn all(&self) -> Result<Vec<Tag>> {
        Ok(tag::Entity::find()
            .order_by_asc(name_order())
            .all(&self.connection)
            .await?
            .into_iter()
            .map(into_model)
            .collect())
    }

    pub async fn get(&self, id: i32) -> Result<Option<Tag>> {
        Ok(tag::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .map(into_model))
    }

    pub async fn for_set(&self, id: i32) -> Result<Vec<Tag>> {
        Ok(tag::Entity::find()
            .filter(
                tag::Column::Id.in_subquery(
                    beatmap_set_tag::Entity::find()
                        .select_only()
                        .column(beatmap_set_tag::Column::TagId)
                        .filter(beatmap_set_tag::Column::BeatmapSetId.eq(id))
                        .into_query(),
                ),
            )
            .order_by_asc(name_order())
            .all(&self.connection)
            .await?
            .into_iter()
            .map(into_model)
            .collect())
    }

    /// Normalizes before both insertion and lookup. Empty names are ignored.
    pub async fn get_or_insert(&self, name: &str) -> Result<Option<Tag>> {
        let name = name.trim().to_lowercase();
        if name.is_empty() {
            return Ok(None);
        }
        tag::Entity::insert(tag::ActiveModel {
            name: Set(name.clone()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(tag::Column::Name)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&self.connection)
        .await?;
        Ok(Some(into_model(
            tag::Entity::find()
                .filter(tag::Column::Name.eq(name))
                .one(&self.connection)
                .await?
                .context("tag missing after insertion")?,
        )))
    }

    pub async fn link(&self, beatmap_set_id: i32, tag_id: i32) -> Result<()> {
        beatmap_set_tag::Entity::insert(beatmap_set_tag::ActiveModel {
            beatmap_set_id: Set(beatmap_set_id),
            tag_id: Set(tag_id),
        })
        .on_conflict(
            OnConflict::columns([
                beatmap_set_tag::Column::BeatmapSetId,
                beatmap_set_tag::Column::TagId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&self.connection)
        .await?;
        Ok(())
    }

    pub async fn cleanup(&self) -> Result<()> {
        tag::Entity::delete_many()
            .filter(
                Expr::exists(
                    beatmap_set_tag::Entity::find()
                        .select_only()
                        .column(beatmap_set_tag::Column::TagId)
                        .filter(
                            Expr::col((beatmap_set_tag::Entity, beatmap_set_tag::Column::TagId))
                                .equals((tag::Entity, tag::Column::Id)),
                        )
                        .into_query(),
                )
                .not(),
            )
            .exec(&self.connection)
            .await?;
        Ok(())
    }
}

fn name_order() -> sea_orm::sea_query::Expr {
    #[cfg(feature = "sqlite")]
    let expression = "name COLLATE BINARY";
    #[cfg(feature = "postgres")]
    let expression = "name COLLATE \"C\"";
    Expr::cust(expression)
}

fn into_model(value: tag::Model) -> Tag {
    Tag {
        id: value.id,
        name: value.name,
    }
}

#[derive(FromQueryResult)]
struct SetTagRow {
    beatmap_set_id: i32,
    name: String,
}

#[derive(FromQueryResult)]
struct SetIdRow {
    beatmap_set_id: i32,
}
