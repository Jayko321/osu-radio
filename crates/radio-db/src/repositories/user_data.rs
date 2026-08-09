use anyhow::Result;
use chrono::Utc;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, QueryResult, SelectableHelper};
use diesel_async::{AsyncConnection, RunQueryDsl, scoped_futures::ScopedFutureExt};
use radio_core::OsuMarker;

use crate::{
    connection::DatabaseConnection,
    model::{NewOsuInstallation, OsuInstallation, OsuInstallationChanges, USER_DATA_ID, UserData},
    schema::{osu_installations, user_data},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisteredInstallation {
    Created(OsuInstallation),
    AlreadyRegistered(OsuInstallation),
}

impl RegisteredInstallation {
    pub fn installation(&self) -> &OsuInstallation {
        match self {
            Self::Created(installation) | Self::AlreadyRegistered(installation) => installation,
        }
    }

    pub fn into_installation(self) -> OsuInstallation {
        match self {
            Self::Created(installation) | Self::AlreadyRegistered(installation) => installation,
        }
    }

    pub fn was_created(&self) -> bool {
        matches!(self, Self::Created(_))
    }
}

pub async fn ensure_user_data(connection: &mut DatabaseConnection) -> Result<UserData> {
    let stored = user_data::table
        .find(USER_DATA_ID)
        .first(connection)
        .await
        .optional()?;

    if let Some(user_data) = stored {
        return Ok(user_data);
    }

    let created = diesel::insert_into(user_data::table)
        .values(user_data::id.eq(USER_DATA_ID))
        .returning(UserData::as_returning())
        .get_result(connection)
        .await?;

    Ok(created)
}

pub async fn all_installations(
    connection: &mut DatabaseConnection,
) -> Result<Vec<OsuInstallation>> {
    let installations = osu_installations::table
        .order(osu_installations::id.asc())
        .select(OsuInstallation::as_select())
        .load(connection)
        .await?;

    Ok(installations)
}

pub async fn installation(
    connection: &mut DatabaseConnection,
    id: i32,
) -> Result<Option<OsuInstallation>> {
    let installation = osu_installations::table
        .find(id)
        .select(OsuInstallation::as_select())
        .first(connection)
        .await
        .optional()?;

    Ok(installation)
}

pub async fn register_installation(
    connection: &mut DatabaseConnection,
    marker: &OsuMarker,
    label: Option<&str>,
) -> Result<RegisteredInstallation> {
    let marker_path = path_string(&marker.marker_path);
    let root_path = path_string(&marker.root_path);

    let registered = connection
        .transaction::<_, diesel::result::Error, _>(|connection| {
            async move {
                ensure_user_data_row(connection).await?;

                let stored = find_by_marker(connection, &marker_path).await?;
                if let Some(installation) = stored {
                    return Ok(RegisteredInstallation::AlreadyRegistered(installation));
                }

                let created = diesel::insert_into(osu_installations::table)
                    .values(NewOsuInstallation {
                        user_data_id: USER_DATA_ID,
                        kind: marker.kind.as_str(),
                        root_path: &root_path,
                        marker_path: &marker_path,
                        label,
                        enabled: true,
                    })
                    .returning(OsuInstallation::as_returning())
                    .get_result(connection)
                    .await?;

                Ok(RegisteredInstallation::Created(created))
            }
            .scope_boxed()
        })
        .await?;

    Ok(registered)
}

pub async fn update_installation(
    connection: &mut DatabaseConnection,
    id: i32,
    changes: OsuInstallationChanges<'_>,
) -> Result<Option<OsuInstallation>> {
    if changes.is_empty() {
        return installation(connection, id).await;
    }

    let updated = diesel::update(osu_installations::table.find(id))
        .set(changes)
        .returning(OsuInstallation::as_returning())
        .get_result(connection)
        .await
        .optional()?;

    Ok(updated)
}

pub async fn delete_installation(connection: &mut DatabaseConnection, id: i32) -> Result<bool> {
    let deleted = diesel::delete(osu_installations::table.find(id))
        .execute(connection)
        .await?;

    Ok(deleted > 0)
}

pub async fn mark_scanned(connection: &mut DatabaseConnection, id: i32) -> Result<()> {
    diesel::update(osu_installations::table.find(id))
        .set(osu_installations::last_scanned_at.eq(Utc::now().to_rfc3339()))
        .execute(connection)
        .await?;

    Ok(())
}

async fn ensure_user_data_row(connection: &mut DatabaseConnection) -> QueryResult<()> {
    let stored: Option<i32> = user_data::table
        .find(USER_DATA_ID)
        .select(user_data::id)
        .first(connection)
        .await
        .optional()?;

    if stored.is_none() {
        diesel::insert_into(user_data::table)
            .values(user_data::id.eq(USER_DATA_ID))
            .execute(connection)
            .await?;
    }

    Ok(())
}

async fn find_by_marker(
    connection: &mut DatabaseConnection,
    marker_path: &str,
) -> QueryResult<Option<OsuInstallation>> {
    osu_installations::table
        .filter(osu_installations::marker_path.eq(marker_path))
        .select(OsuInstallation::as_select())
        .first(connection)
        .await
        .optional()
}

fn path_string(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use diesel_async::{AsyncConnection, SimpleAsyncConnection};
    use radio_core::OsuKind;

    use super::*;

    async fn connection() -> DatabaseConnection {
        let mut connection = DatabaseConnection::establish(":memory:")
            .await
            .expect("database connection should open");
        connection
            .batch_execute(crate::CREATE_SCHEMA)
            .await
            .expect("initial schema migration should apply");

        connection
    }

    fn marker(root: &str) -> OsuMarker {
        OsuMarker {
            kind: OsuKind::Lazer,
            marker_path: PathBuf::from(root).join("client.realm"),
            root_path: PathBuf::from(root),
        }
    }

    #[tokio::test]
    async fn the_schema_creates_the_singleton_settings_row() {
        let mut connection = connection().await;

        let user_data = ensure_user_data(&mut connection)
            .await
            .expect("user data should load");

        assert_eq!(user_data.id, USER_DATA_ID);
    }

    #[tokio::test]
    async fn registering_the_same_marker_twice_returns_the_stored_installation() {
        let mut connection = connection().await;

        let created = register_installation(&mut connection, &marker("/osu"), Some("Desktop"))
            .await
            .expect("installation should register");
        assert!(created.was_created());

        let again = register_installation(&mut connection, &marker("/osu"), Some("Other"))
            .await
            .expect("installation should resolve");

        assert!(!again.was_created());
        assert_eq!(again.installation().id, created.installation().id);
        assert_eq!(again.installation().label.as_deref(), Some("Desktop"));
        assert_eq!(
            all_installations(&mut connection)
                .await
                .expect("installations should load")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn a_registered_installation_keeps_its_kind_and_paths() {
        let mut connection = connection().await;
        register_installation(&mut connection, &marker("/osu"), None)
            .await
            .expect("installation should register");

        let stored = all_installations(&mut connection)
            .await
            .expect("installations should load");

        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].kind, OsuKind::Lazer);
        assert_eq!(stored[0].root_path, "/osu");
        assert!(stored[0].marker_path.ends_with("client.realm"));
        assert_eq!(stored[0].label, None);
        assert!(stored[0].enabled);
        assert_eq!(stored[0].last_scanned_at, None);
    }

    #[tokio::test]
    async fn updating_changes_only_the_supplied_fields() {
        let mut connection = connection().await;
        let id = register_installation(&mut connection, &marker("/osu"), Some("Desktop"))
            .await
            .expect("installation should register")
            .into_installation()
            .id;

        let updated = update_installation(
            &mut connection,
            id,
            OsuInstallationChanges {
                enabled: Some(false),
                ..OsuInstallationChanges::default()
            },
        )
        .await
        .expect("installation should update")
        .expect("installation should exist");

        assert!(!updated.enabled);
        assert_eq!(updated.label.as_deref(), Some("Desktop"));

        let cleared = update_installation(
            &mut connection,
            id,
            OsuInstallationChanges {
                label: Some(None),
                ..OsuInstallationChanges::default()
            },
        )
        .await
        .expect("installation should update")
        .expect("installation should exist");

        assert_eq!(cleared.label, None);
        assert!(!cleared.enabled);
    }

    #[tokio::test]
    async fn updating_or_deleting_an_unknown_installation_reports_it_is_missing() {
        let mut connection = connection().await;

        let updated = update_installation(
            &mut connection,
            404,
            OsuInstallationChanges {
                enabled: Some(false),
                ..OsuInstallationChanges::default()
            },
        )
        .await
        .expect("update should run");

        assert!(updated.is_none());
        assert!(
            !delete_installation(&mut connection, 404)
                .await
                .expect("delete should run")
        );
    }

    #[tokio::test]
    async fn marking_a_scan_stores_an_rfc_3339_timestamp() {
        let mut connection = connection().await;
        let id = register_installation(&mut connection, &marker("/osu"), None)
            .await
            .expect("installation should register")
            .into_installation()
            .id;

        mark_scanned(&mut connection, id)
            .await
            .expect("scan should be marked");

        let stored = installation(&mut connection, id)
            .await
            .expect("installation should load")
            .expect("installation should exist");
        let last_scanned_at = stored
            .last_scanned_at
            .expect("a scanned installation should carry a timestamp");

        assert!(
            chrono::DateTime::parse_from_rfc3339(&last_scanned_at).is_ok(),
            "{last_scanned_at} should be RFC 3339"
        );
    }
}
