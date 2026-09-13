use crate::{
    entities::osu_installation,
    model::{OsuInstallation, OsuInstallationChanges, USER_DATA_ID},
};
use anyhow::{Context, Result};
use chrono::Utc;
use radio_core::OsuMarker;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set, sea_query::OnConflict,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisteredInstallation {
    Created(OsuInstallation),
    AlreadyRegistered(OsuInstallation),
}

impl RegisteredInstallation {
    #[must_use]
    pub const fn installation(&self) -> &OsuInstallation {
        match self {
            Self::Created(installation) | Self::AlreadyRegistered(installation) => installation,
        }
    }

    #[must_use]
    pub fn into_installation(self) -> OsuInstallation {
        match self {
            Self::Created(installation) | Self::AlreadyRegistered(installation) => installation,
        }
    }

    #[must_use]
    pub const fn was_created(&self) -> bool {
        matches!(self, Self::Created(_))
    }
}

pub struct OsuInstallationRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl OsuInstallationRepository<'_> {
    pub async fn all(&self) -> Result<Vec<OsuInstallation>> {
        osu_installation::Entity::find()
            .order_by_asc(osu_installation::Column::Id)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(model)
            .collect()
    }

    pub async fn get(&self, id: i32) -> Result<Option<OsuInstallation>> {
        get(&self.connection, id).await
    }

    pub async fn register(
        &self,
        marker: &OsuMarker,
        label: Option<&str>,
    ) -> Result<RegisteredInstallation> {
        let marker_path = marker.marker_path.to_string_lossy().into_owned();
        let inserted = osu_installation::Entity::insert(osu_installation::ActiveModel {
            user_data_id: Set(USER_DATA_ID),
            kind: Set(marker.kind.as_str().to_owned()),
            root_path: Set(marker.root_path.to_string_lossy().into_owned()),
            marker_path: Set(marker_path.clone()),
            label: Set(label.map(str::to_owned)),
            enabled: Set(true),
            last_scanned_at: Set(None),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(osu_installation::Column::MarkerPath)
                .do_nothing()
                .to_owned(),
        )
        .try_insert()
        .exec(&self.connection)
        .await?;
        let stored = osu_installation::Entity::find()
            .filter(osu_installation::Column::MarkerPath.eq(marker_path))
            .one(&self.connection)
            .await?
            .context("Registered installation disappeared")?;
        let stored = model(stored)?;
        Ok(
            if matches!(inserted, sea_orm::TryInsertResult::Inserted(_)) {
                RegisteredInstallation::Created(stored)
            } else {
                RegisteredInstallation::AlreadyRegistered(stored)
            },
        )
    }

    pub async fn update(
        &self,
        id: i32,
        changes: OsuInstallationChanges<'_>,
    ) -> Result<Option<OsuInstallation>> {
        if changes.is_empty() {
            return self.get(id).await;
        }
        // Update only specified columns so concurrent partial changes cannot overwrite each other.
        let mut update =
            osu_installation::Entity::update_many().filter(osu_installation::Column::Id.eq(id));
        if let Some(label) = changes.label {
            update = update.col_expr(
                osu_installation::Column::Label,
                sea_orm::sea_query::Expr::value(label.map(str::to_owned)),
            );
        }
        if let Some(enabled) = changes.enabled {
            update = update.col_expr(
                osu_installation::Column::Enabled,
                sea_orm::sea_query::Expr::value(enabled),
            );
        }
        update.exec(&self.connection).await?;
        self.get(id).await
    }

    /// Deletes the installation using schema cascades; the service coordinates shared cleanup.
    pub async fn delete(&self, id: i32) -> Result<bool> {
        Ok(osu_installation::Entity::delete_by_id(id)
            .exec(&self.connection)
            .await?
            .rows_affected
            > 0)
    }

    pub async fn mark_scanned(&self, id: i32) -> Result<()> {
        osu_installation::Entity::update_many()
            .col_expr(
                osu_installation::Column::LastScannedAt,
                sea_orm::sea_query::Expr::value(Utc::now().to_rfc3339()),
            )
            .filter(osu_installation::Column::Id.eq(id))
            .exec(&self.connection)
            .await?;
        Ok(())
    }
}

async fn get(connection: &impl ConnectionTrait, id: i32) -> Result<Option<OsuInstallation>> {
    osu_installation::Entity::find_by_id(id)
        .one(connection)
        .await?
        .map(model)
        .transpose()
}

fn model(row: osu_installation::Model) -> Result<OsuInstallation> {
    Ok(OsuInstallation {
        id: row.id,
        user_data_id: row.user_data_id,
        kind: row.kind.parse()?,
        root_path: row.root_path,
        marker_path: row.marker_path,
        label: row.label,
        enabled: row.enabled,
        last_scanned_at: row.last_scanned_at,
    })
}
