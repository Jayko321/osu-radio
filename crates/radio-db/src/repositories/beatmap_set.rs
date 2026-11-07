use anyhow::{Context, Result};
use radio_core::import_types::ImportedBeatmapSet;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, Set,
    sea_query::{Alias, Expr, ExprTrait, Order, Query},
};
use std::collections::{BTreeMap, HashSet};

use crate::{
    entities::{audio_source, beatmap, beatmap_metadata, beatmap_set},
    model::{AudioSource, BeatmapDetails, BeatmapSet, BeatmapSetWithAudio, SourceType},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextField {
    Any,
    Artist,
    Title,
    Mapper,
    Difficulty,
    Source,
    Tag,
}

#[derive(Debug)]
pub struct TextCriterion {
    pub field: TextField,
    pub value: String,
    pub exact: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericField {
    Bpm,
    Stars,
    Length,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug)]
pub enum NumericValue {
    Number(f64),
    Milliseconds { whole: i64, fractional: bool },
}

#[derive(Clone, Copy, Debug)]
pub struct NumericCriterion {
    pub field: NumericField,
    pub comparison: Comparison,
    pub value: NumericValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateField {
    Ranked,
    Submitted,
}

#[derive(Clone, Copy, Debug)]
pub struct DateCriterion {
    pub field: DateField,
    pub comparison: Comparison,
    pub start: i64,
    pub end: i64,
}

#[derive(FromQueryResult)]
struct TextRow {
    id: i32,
    value: String,
}

const fn comparison_sql(comparison: Comparison) -> &'static str {
    match comparison {
        Comparison::Equal => "=",
        Comparison::NotEqual => "<>",
        Comparison::Less => "<",
        Comparison::LessEqual => "<=",
        Comparison::Greater => ">",
        Comparison::GreaterEqual => ">=",
    }
}

pub struct BeatmapSetRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl BeatmapSetRepository<'_> {
    /// Streams only SQL-matched values for phrase verification; IDs are global audio IDs.
    pub async fn matching_text<F>(
        &self,
        criterion: &TextCriterion,
        verify_values: bool,
        mut verify: F,
    ) -> Result<Vec<i32>>
    where
        F: FnMut(&str) -> bool + Send,
    {
        #[cfg(feature = "sqlite")]
        let (parameter, contains) = ("?", "instr");
        #[cfg(feature = "postgres")]
        let (parameter, contains) = ("$1", "strpos");
        let fields: &[&str] = match criterion.field {
            TextField::Any => &[
                "m.search_artist",
                "m.search_artist_unicode",
                "m.search_title",
                "m.search_title_unicode",
                "m.search_mapper",
                "m.search_source",
                "b.search_difficulty_name",
                "t.name",
            ],
            TextField::Artist => &["m.search_artist", "m.search_artist_unicode"],
            TextField::Title => &["m.search_title", "m.search_title_unicode"],
            TextField::Mapper => &["m.search_mapper"],
            TextField::Source => &["m.search_source"],
            TextField::Difficulty => &["b.search_difficulty_name"],
            TextField::Tag => &["t.name"],
        };
        let predicate = |field: &str| {
            if criterion.exact {
                format!("{field} = needle.word")
            } else {
                format!("{contains}({field}, needle.word) > 0")
            }
        };
        let branches = fields.iter().map(|field| {
            let joins = if *field == "t.name" {
                "JOIN beatmap_set_tags st ON st.beatmap_set_id = b.beatmap_set_id JOIN tags t ON t.id = st.tag_id"
            } else if field.starts_with("m.") {
                "JOIN beatmap_metadata m ON m.hash = b.metadata_hash"
            } else { "" };
            let value = if verify_values { format!(", {field} AS value") } else { String::new() };
            format!("SELECT b.audio_source_id AS id{value} FROM beatmaps b {joins}, needle WHERE b.audio_source_id IS NOT NULL AND {}", predicate(field))
        }).collect::<Vec<_>>().join(" UNION ALL ");
        let statement = sea_orm::Statement::from_sql_and_values(
            self.connection.get_database_backend(),
            format!(
                "WITH needle(word) AS (VALUES (CAST({parameter} AS TEXT))) SELECT DISTINCT * FROM ({branches}) AS matches"
            ),
            [criterion.value.clone().into()],
        );
        let mut ids = HashSet::new();
        if verify_values {
            super::query_each::<TextRow>(&self.connection, statement, |row| {
                if verify(&row.value) {
                    ids.insert(row.id);
                }
                Ok(())
            })
            .await?;
        } else {
            super::query_each::<IdRow>(&self.connection, statement, |row| {
                ids.insert(row.id);
                Ok(())
            })
            .await?;
        }
        let mut ids: Vec<_> = ids.into_iter().collect();
        ids.sort_unstable();
        Ok(ids)
    }

    /// Backwards-compatible literal free-text search.
    pub async fn matching_audio_sources(&self, word: &str) -> Result<Vec<i32>> {
        self.matching_text(
            &TextCriterion {
                field: TextField::Any,
                value: word.to_owned(),
                exact: false,
            },
            false,
            |_| true,
        )
        .await
    }

    pub async fn matching_online_set_id(&self, online_id: i32) -> Result<Vec<i32>> {
        #[cfg(feature = "sqlite")]
        let parameter = "?";
        #[cfg(feature = "postgres")]
        let parameter = "$1";
        let statement = sea_orm::Statement::from_sql_and_values(
            self.connection.get_database_backend(),
            format!(
                "SELECT DISTINCT b.audio_source_id AS id FROM beatmaps b JOIN beatmap_sets s ON s.id = b.beatmap_set_id WHERE b.audio_source_id IS NOT NULL AND s.online_id = {parameter}"
            ),
            [online_id.into()],
        );
        let mut ids = Vec::new();
        super::query_each::<IdRow>(&self.connection, statement, |row| {
            ids.push(row.id);
            Ok(())
        })
        .await?;
        Ok(ids)
    }

    /// Conditions for one field apply to the same known source value.
    pub async fn matching_numeric(
        &self,
        field: NumericField,
        criteria: &[NumericCriterion],
    ) -> Result<Vec<i32>> {
        // A float already equal to a hundredth must not fall one cent below it after multiplication.
        #[cfg(feature = "sqlite")]
        let stars = "CASE WHEN ABS(b.stars) >= 90071992547409.92 THEN b.stars WHEN b.stars = ROUND(b.stars * 100.0) / 100.0 THEN b.stars ELSE (CAST(b.stars * 100 AS INTEGER) - CASE WHEN b.stars * 100 < CAST(b.stars * 100 AS INTEGER) THEN 1 ELSE 0 END) / 100.0 END";
        #[cfg(feature = "postgres")]
        let stars = "CASE WHEN ABS(b.stars) >= 90071992547409.92 THEN b.stars WHEN b.stars = ROUND(b.stars * 100.0) / 100.0 THEN b.stars ELSE FLOOR(b.stars * 100.0) / 100.0 END";
        let (column, source) = match field {
            NumericField::Bpm => ("b.bpm", "beatmaps b"),
            NumericField::Stars => (stars, "beatmaps b"),
            NumericField::Length => (
                "a.duration_ms",
                "beatmaps b JOIN audio_sources a ON a.id = b.audio_source_id",
            ),
        };
        let known = match field {
            NumericField::Bpm => "b.bpm",
            NumericField::Stars => "b.stars",
            NumericField::Length => "a.duration_ms",
        };
        let mut values: Vec<sea_orm::Value> = Vec::new();
        let mut predicates = vec![format!("{known} IS NOT NULL")];
        for criterion in criteria {
            let mut bind = |value: sea_orm::Value| {
                values.push(value);
                #[cfg(feature = "sqlite")]
                {
                    "?".to_owned()
                }
                #[cfg(feature = "postgres")]
                {
                    format!("${}", values.len())
                }
            };
            let op = criterion.comparison;
            let predicate = match criterion.value {
                NumericValue::Number(n)
                    if field == NumericField::Bpm
                        && matches!(op, Comparison::Equal | Comparison::NotEqual) =>
                {
                    let lower = bind((n - 0.5).into());
                    let upper = bind((n + 0.5).into());
                    if op == Comparison::Equal {
                        format!("{column} BETWEEN {lower} AND {upper}")
                    } else {
                        format!("({column} < {lower} OR {column} > {upper})")
                    }
                }
                NumericValue::Number(n) => {
                    format!("{column} {} {}", comparison_sql(op), bind(n.into()))
                }
                NumericValue::Milliseconds { whole, fractional } => match (op, fractional) {
                    (Comparison::Equal, true) => "1 = 0".into(),
                    (Comparison::NotEqual, true) => "1 = 1".into(),
                    (Comparison::Less, true) => format!("{column} <= {}", bind(whole.into())),
                    (Comparison::GreaterEqual, true) => {
                        format!("{column} > {}", bind(whole.into()))
                    }
                    _ => format!("{column} {} {}", comparison_sql(op), bind(whole.into())),
                },
            };
            predicates.push(predicate);
        }
        let sql = format!(
            "SELECT DISTINCT b.audio_source_id AS id FROM {source} WHERE b.audio_source_id IS NOT NULL AND {}",
            predicates.join(" AND ")
        );
        let statement = sea_orm::Statement::from_sql_and_values(
            self.connection.get_database_backend(),
            sql,
            values,
        );
        let mut ids = Vec::new();
        super::query_each::<IdRow>(&self.connection, statement, |row| {
            ids.push(row.id);
            Ok(())
        })
        .await?;
        Ok(ids)
    }

    pub async fn matching_status(&self, statuses: &[String]) -> Result<Vec<i32>> {
        let mut values = Vec::new();
        let mut predicates = Vec::new();
        for status in statuses {
            if status == "none" {
                predicates.push("b.status IS NULL".to_owned());
            } else {
                values.push(status.clone().into());
                #[cfg(feature = "sqlite")]
                let placeholder = "?".to_owned();
                #[cfg(feature = "postgres")]
                let placeholder = format!("${}", values.len());
                predicates.push(format!("b.status = {placeholder}"));
            }
        }
        self.search_ids(format!("SELECT DISTINCT b.audio_source_id AS id FROM beatmaps b WHERE b.audio_source_id IS NOT NULL AND ({})", predicates.join(" OR ")), values).await
    }

    pub async fn matching_dates(
        &self,
        field: DateField,
        criteria: &[DateCriterion],
    ) -> Result<Vec<i32>> {
        let column = match field {
            DateField::Ranked => "s.ranked_at_ms",
            DateField::Submitted => "s.submitted_at_ms",
        };
        let mut values = Vec::new();
        let mut predicates = vec![format!("{column} IS NOT NULL")];
        for criterion in criteria {
            let mut bind = |v: i64| {
                values.push(v.into());
                #[cfg(feature = "sqlite")]
                {
                    "?".to_owned()
                }
                #[cfg(feature = "postgres")]
                {
                    format!("${}", values.len())
                }
            };
            let predicate = match criterion.comparison {
                Comparison::Equal => format!(
                    "({column} >= {} AND {column} < {})",
                    bind(criterion.start),
                    bind(criterion.end)
                ),
                Comparison::NotEqual => format!(
                    "({column} < {} OR {column} >= {})",
                    bind(criterion.start),
                    bind(criterion.end)
                ),
                Comparison::Less => format!("{column} < {}", bind(criterion.start)),
                Comparison::LessEqual => format!("{column} < {}", bind(criterion.end)),
                Comparison::Greater => format!("{column} >= {}", bind(criterion.end)),
                Comparison::GreaterEqual => format!("{column} >= {}", bind(criterion.start)),
            };
            predicates.push(predicate);
        }
        self.search_ids(format!("SELECT DISTINCT b.audio_source_id AS id FROM beatmaps b JOIN beatmap_sets s ON s.id = b.beatmap_set_id WHERE b.audio_source_id IS NOT NULL AND {}", predicates.join(" AND ")), values).await
    }

    pub async fn matching_played(
        &self,
        played: &[bool],
        ages: &[(Comparison, i64, bool)],
        now: i64,
    ) -> Result<Vec<i32>> {
        let mut values: Vec<sea_orm::Value> = Vec::new();
        let mut predicates = Vec::new();
        for value in played {
            predicates.push(
                if *value {
                    "MAX(b.osu_last_played_at_ms) IS NOT NULL"
                } else {
                    "MAX(b.osu_last_played_at_ms) IS NULL"
                }
                .to_owned(),
            );
        }
        for &(op, whole, fractional) in ages {
            predicates.push("MAX(b.osu_last_played_at_ms) IS NOT NULL".to_owned());
            // Compare the timestamp against now minus the age, reversing the comparison.
            let Some(threshold) = now.checked_sub(whole) else {
                if matches!(
                    op,
                    Comparison::Equal | Comparison::Greater | Comparison::GreaterEqual
                ) {
                    predicates.push("1 = 0".into());
                }
                continue;
            };
            let relation = match (op, fractional) {
                (Comparison::Equal, true) => {
                    predicates.push("1 = 0".into());
                    continue;
                }
                (Comparison::NotEqual, true) => {
                    continue;
                }
                (Comparison::Equal, false) => "=",
                (Comparison::NotEqual, false) => "<>",
                (Comparison::Less, true) | (Comparison::LessEqual, _) => ">=",
                (Comparison::Less, false) => ">",
                (Comparison::Greater, _) | (Comparison::GreaterEqual, true) => "<",
                (Comparison::GreaterEqual, false) => "<=",
            };
            values.push(threshold.into());
            #[cfg(feature = "sqlite")]
            let placeholder = "?".to_owned();
            #[cfg(feature = "postgres")]
            let placeholder = format!("${}", values.len());
            predicates.push(format!(
                "MAX(b.osu_last_played_at_ms) {relation} {placeholder}"
            ));
        }
        self.search_ids(format!("SELECT b.audio_source_id AS id FROM beatmaps b WHERE b.audio_source_id IS NOT NULL GROUP BY b.audio_source_id HAVING {}", predicates.join(" AND ")), values).await
    }

    async fn search_ids(&self, sql: String, values: Vec<sea_orm::Value>) -> Result<Vec<i32>> {
        let statement = sea_orm::Statement::from_sql_and_values(
            self.connection.get_database_backend(),
            sql,
            values,
        );
        let mut ids = Vec::new();
        super::query_each::<IdRow>(&self.connection, statement, |row| {
            ids.push(row.id);
            Ok(())
        })
        .await?;
        Ok(ids)
    }

    pub async fn audio_source_ids(&self) -> Result<Vec<i32>> {
        let statement = sea_orm::Statement::from_string(
            self.connection.get_database_backend(),
            "SELECT DISTINCT audio_source_id AS id FROM beatmaps WHERE audio_source_id IS NOT NULL ORDER BY audio_source_id",
        );
        let mut ids = Vec::new();
        super::query_each::<IdRow>(&self.connection, statement, |row| {
            ids.push(row.id);
            Ok(())
        })
        .await?;
        Ok(ids)
    }

    pub async fn multiple_audio_set_ids(&self) -> Result<HashSet<i32>> {
        let statement = sea_orm::Statement::from_string(
            self.connection.get_database_backend(),
            "SELECT beatmap_set_id AS id FROM beatmaps GROUP BY beatmap_set_id \
             HAVING COUNT(DISTINCT audio_source_id) > 1",
        );
        let mut ids = HashSet::new();
        super::query_each::<IdRow>(&self.connection, statement, |row| {
            ids.insert(row.id);
            Ok(())
        })
        .await?;
        Ok(ids)
    }

    pub async fn insert(
        &self,
        installation_id: i32,
        imported: &ImportedBeatmapSet,
    ) -> Result<BeatmapSet> {
        Ok(into_model(
            beatmap_set::ActiveModel {
                online_id: Set(imported.online_id),
                hash: Set(imported.hash.clone()),
                ranked_at_ms: Set(imported.ranked_at_ms),
                submitted_at_ms: Set(imported.submitted_at_ms),
                installation_id: Set(installation_id),
                ..Default::default()
            }
            .insert(&self.connection)
            .await?,
        ))
    }
    pub async fn delete_for_installation(&self, installation_id: i32) -> Result<()> {
        beatmap_set::Entity::delete_many()
            .filter(beatmap_set::Column::InstallationId.eq(installation_id))
            .exec(&self.connection)
            .await?;
        Ok(())
    }

    pub async fn get(&self, id: i32) -> Result<Option<BeatmapSet>> {
        Ok(beatmap_set::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .map(into_model))
    }

    pub async fn for_installation(&self, installation_id: i32) -> Result<Vec<BeatmapSet>> {
        Ok(beatmap_set::Entity::find()
            .filter(beatmap_set::Column::InstallationId.eq(installation_id))
            .order_by_asc(beatmap_set::Column::Id)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(into_model)
            .collect())
    }

    pub async fn search_difficulties(&self) -> Result<Vec<crate::model::SearchDifficulty>> {
        let statement = sea_orm::Statement::from_string(
            self.connection.get_database_backend(),
            "SELECT beatmap_set_id AS set_id, audio_source_id, metadata_hash, difficulty_name \
             FROM beatmaps WHERE audio_source_id IS NOT NULL",
        );
        Ok(
            super::query_all::<SearchDifficultyRow>(&self.connection, statement)
                .await?
                .into_iter()
                .map(|row| crate::model::SearchDifficulty {
                    set_id: row.set_id,
                    audio_source_id: row.audio_source_id,
                    metadata_hash: row.metadata_hash,
                    difficulty_name: row.difficulty_name,
                })
                .collect(),
        )
    }

    pub async fn all_with_audio_sources(&self) -> Result<Vec<BeatmapSetWithAudio>> {
        self.load_with_audio_sources(None).await
    }

    /// One track's references and multiplicity, without loading unrelated library data.
    pub async fn for_audio_source(&self, id: i32) -> Result<Vec<BeatmapSetWithAudio>> {
        let mut sets = self.load_with_audio_sources(Some(&[id])).await?;
        for set in &mut sets {
            set.has_multiple_audio_sources = beatmap::Entity::find()
                .filter(beatmap::Column::BeatmapSetId.eq(set.beatmap_set.id))
                .filter(beatmap::Column::AudioSourceId.ne(id))
                .one(&self.connection)
                .await?
                .is_some();
        }
        Ok(sets)
    }

    /// Caller supplies multiplicity from the unfiltered search snapshot.
    /// Chunking is safe only on a transaction-bound repository for a consistent read.
    pub async fn for_audio_sources(
        &self,
        ids: &[i32],
        multiple_audio_sets: &std::collections::HashSet<i32>,
    ) -> Result<Vec<BeatmapSetWithAudio>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut sets = self.load_with_audio_sources(Some(ids)).await?;
        for set in &mut sets {
            set.has_multiple_audio_sources = multiple_audio_sets.contains(&set.beatmap_set.id);
        }
        Ok(sets)
    }

    #[allow(clippy::too_many_lines)] // Keep the aggregate query and materialization together.
    async fn load_with_audio_sources(
        &self,
        ids: Option<&[i32]>,
    ) -> Result<Vec<BeatmapSetWithAudio>> {
        use audio_source::Column as Audio;
        use beatmap::Column as Map;
        use beatmap_metadata::Column as Meta;
        use beatmap_set::Column as SetColumn;

        // Blank queries use one aggregate query; filtered callers hold a read transaction.
        let query = Query::select()
            .columns([
                (beatmap_set::Entity, SetColumn::Id),
                (beatmap_set::Entity, SetColumn::OnlineId),
                (beatmap_set::Entity, SetColumn::Hash),
                (beatmap_set::Entity, SetColumn::RankedAtMs),
                (beatmap_set::Entity, SetColumn::SubmittedAtMs),
                (beatmap_set::Entity, SetColumn::InstallationId),
            ])
            .expr_as(
                Expr::col((audio_source::Entity, Audio::Id)),
                Alias::new("audio_id"),
            )
            .expr_as(
                Expr::col((audio_source::Entity, Audio::Kind)),
                Alias::new("audio_kind"),
            )
            .expr_as(
                Expr::col((audio_source::Entity, Audio::Location)),
                Alias::new("audio_location"),
            )
            .expr_as(
                Expr::col((audio_source::Entity, Audio::DurationMs)),
                Alias::new("audio_duration_ms"),
            )
            .expr_as(
                Expr::col((beatmap::Entity, Map::OsuLastPlayedAtMs)),
                Alias::new("osu_last_played_at_ms"),
            )
            .expr_as(Expr::col((beatmap::Entity, Map::Id)), Alias::new("map_id"))
            .expr_as(
                Expr::col((beatmap::Entity, Map::DifficultyName)),
                Alias::new("difficulty_name"),
            )
            .expr_as(
                Expr::col((beatmap::Entity, Map::BackgroundPath)).is_not_null(),
                Alias::new("has_cover"),
            )
            .columns([
                (beatmap_metadata::Entity, Meta::Title),
                (beatmap_metadata::Entity, Meta::TitleUnicode),
                (beatmap_metadata::Entity, Meta::Artist),
                (beatmap_metadata::Entity, Meta::ArtistUnicode),
            ])
            .from(beatmap_set::Entity)
            .left_join(
                beatmap::Entity,
                Expr::col((beatmap_set::Entity, SetColumn::Id))
                    .equals((beatmap::Entity, Map::BeatmapSetId)),
            )
            .left_join(
                audio_source::Entity,
                Expr::col((beatmap::Entity, Map::AudioSourceId))
                    .equals((audio_source::Entity, Audio::Id)),
            )
            .left_join(
                beatmap_metadata::Entity,
                Expr::col((beatmap::Entity, Map::MetadataHash))
                    .equals((beatmap_metadata::Entity, Meta::Hash)),
            )
            .order_by((beatmap_set::Entity, SetColumn::Id), Order::Asc)
            .order_by((audio_source::Entity, Audio::Id), Order::Asc)
            .order_by((beatmap::Entity, Map::Id), Order::Asc)
            .to_owned();
        let mut sets = BTreeMap::new();
        let mut consume = |row: SetWithAudio| -> Result<()> {
            let set = sets.entry(row.id).or_insert_with(|| BeatmapSetWithAudio {
                beatmap_set: BeatmapSet {
                    id: row.id,
                    online_id: row.online_id,
                    hash: row.hash,
                    ranked_at_ms: row.ranked_at_ms,
                    submitted_at_ms: row.submitted_at_ms,
                    installation_id: row.installation_id,
                },
                has_multiple_audio_sources: false,
                audio_sources: Vec::new(),
                beatmaps: Vec::new(),
            });
            if let Some(id) = row.audio_id
                && set.audio_sources.last().is_none_or(|audio| audio.id != id)
            {
                set.audio_sources.push(AudioSource {
                    id,
                    s_type: SourceType::from_parts(
                        &row.audio_kind.context("joined audio source has no kind")?,
                        row.audio_location
                            .context("joined audio source has no location")?,
                    )?,
                    duration_ms: row.audio_duration_ms,
                });
            }
            if let Some(id) = row.map_id {
                set.beatmaps.push(BeatmapDetails {
                    osu_last_played_at_ms: row.osu_last_played_at_ms,
                    id,
                    audio_source_id: row.audio_id,
                    difficulty_name: row.difficulty_name,
                    title: row.title,
                    title_unicode: row.title_unicode,
                    artist: row.artist,
                    artist_unicode: row.artist_unicode,
                    has_cover: row.has_cover,
                });
            }
            Ok(())
        };
        let backend = self.connection.get_database_backend();
        if let Some(ids) = ids {
            let mut ids = ids.to_vec();
            ids.sort_unstable();
            ids.dedup();
            // Disjoint ascending audio chunks preserve each set's audio/map order.
            for chunk in ids.chunks(500) {
                let mut filtered = query.clone();
                filtered.and_where(
                    Expr::col((beatmap::Entity, Map::AudioSourceId)).is_in(chunk.iter().copied()),
                );
                super::query_each(&self.connection, backend.build(&filtered), &mut consume).await?;
            }
        } else {
            super::query_each(&self.connection, backend.build(&query), &mut consume).await?;
        }
        let mut sets: Vec<_> = sets.into_values().collect();
        for set in &mut sets {
            set.has_multiple_audio_sources = set.audio_sources.len() > 1;
        }
        Ok(sets)
    }
}

#[derive(FromQueryResult)]
struct SetWithAudio {
    id: i32,
    online_id: Option<i32>,
    hash: Option<String>,
    ranked_at_ms: Option<i64>,
    submitted_at_ms: Option<i64>,
    installation_id: i32,
    audio_id: Option<i32>,
    audio_kind: Option<String>,
    audio_location: Option<String>,
    audio_duration_ms: Option<i64>,
    map_id: Option<i32>,
    osu_last_played_at_ms: Option<i64>,
    difficulty_name: Option<String>,
    has_cover: bool,
    title: Option<String>,
    title_unicode: Option<String>,
    artist: Option<String>,
    artist_unicode: Option<String>,
}

fn into_model(value: beatmap_set::Model) -> BeatmapSet {
    BeatmapSet {
        id: value.id,
        online_id: value.online_id,
        hash: value.hash,
        ranked_at_ms: value.ranked_at_ms,
        submitted_at_ms: value.submitted_at_ms,
        installation_id: value.installation_id,
    }
}

#[derive(FromQueryResult)]
struct SearchDifficultyRow {
    set_id: i32,
    audio_source_id: i32,
    metadata_hash: Option<String>,
    difficulty_name: Option<String>,
}

#[derive(FromQueryResult)]
struct IdRow {
    id: i32,
}
