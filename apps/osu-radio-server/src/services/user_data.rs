use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use radio_core::OsuMarker;
use radio_db::{
    RegisteredInstallation,
    model::{OsuInstallation, OsuInstallationChanges},
};
use radio_scanner::discovery::{DiscoveryDepth, DiscoveryOptions, discover};

use crate::state::AppState;

#[derive(Debug)]
pub(crate) struct UserDataOverview {
    pub(crate) id: i32,
    pub(crate) osu_folders: Vec<OsuInstallation>,
}

#[derive(Debug)]
pub(crate) enum RegisterFolderError {
    RelativePath(PathBuf),
    NotAnOsuFolder(PathBuf),
    Ambiguous {
        path: PathBuf,
        found: Vec<PathBuf>,
    },
    /// Anything the caller cannot fix by sending a different path.
    Failed(anyhow::Error),
}

#[derive(Debug, Default)]
pub(crate) struct FolderChanges {
    #[allow(clippy::option_option)]
    pub(crate) label: Option<Option<String>>,
    pub(crate) enabled: Option<bool>,
}

pub(crate) struct UserDataService<'a> {
    state: &'a AppState,
}

impl<'a> UserDataService<'a> {
    pub(crate) const fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub(crate) async fn overview(&self) -> Result<UserDataOverview> {
        let database = self.state.database();

        let user_data = database
            .user_data()
            .get()
            .await
            .context("Failed to load the stored user data")?;
        let osu_folders = database
            .osu_installations()
            .all()
            .await
            .context("Failed to load the registered osu! folders")?;

        Ok(UserDataOverview {
            id: user_data.id,
            osu_folders,
        })
    }

    pub(crate) async fn osu_folders(&self) -> Result<Vec<OsuInstallation>> {
        self.state
            .database()
            .osu_installations()
            .all()
            .await
            .context("Failed to load the registered osu! folders")
    }

    pub(crate) async fn register_osu_folder(
        &self,
        path: PathBuf,
        label: Option<String>,
    ) -> Result<RegisteredInstallation, RegisterFolderError> {
        let marker = resolve_osu_folder(&path).await?;

        self.state
            .database()
            .osu_installations()
            .register(&marker, label.as_deref())
            .await
            .context("Failed to register the osu! folder")
            .map_err(RegisterFolderError::Failed)
    }

    pub(crate) async fn update_osu_folder(
        &self,
        id: i32,
        changes: FolderChanges,
    ) -> Result<Option<OsuInstallation>> {
        self.state
            .database()
            .osu_installations()
            .update(
                id,
                OsuInstallationChanges {
                    label: changes
                        .label
                        .as_ref()
                        .map(|label| label.as_deref().map(str::trim)),
                    enabled: changes.enabled,
                },
            )
            .await
            .context("Failed to update the registered osu! folder")
    }

    pub(crate) async fn remove_osu_folder(&self, id: i32) -> Result<bool> {
        self.state
            .database()
            .osu_installations()
            .delete(id)
            .await
            .context("Failed to remove the registered osu! folder")
    }
}

/// Confirms `path` really holds an osu! installation and derives its kind, instead of
/// trusting what the client claimed.
async fn resolve_osu_folder(path: &Path) -> Result<OsuMarker, RegisterFolderError> {
    if !path.is_absolute() {
        return Err(RegisterFolderError::RelativePath(path.to_path_buf()));
    }

    let mut markers = discover(DiscoveryOptions {
        roots: vec![path.to_path_buf()],
        depth: DiscoveryDepth::Known,
        ..DiscoveryOptions::default()
    })
    .collect()
    .await;

    match markers.len() {
        0 => Err(RegisterFolderError::NotAnOsuFolder(path.to_path_buf())),
        1 => Ok(markers.remove(0)),
        _ => Err(RegisterFolderError::Ambiguous {
            path: path.to_path_buf(),
            found: markers
                .into_iter()
                .map(|marker| marker.marker_path)
                .collect(),
        }),
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use radio_core::OsuKind;

    use crate::test_support::{empty_state, lazer_folder, stable_folder};

    use super::*;

    #[tokio::test]
    async fn a_fresh_database_has_the_settings_row_and_no_folders() {
        let state = empty_state().await;

        let overview = UserDataService::new(&state)
            .overview()
            .await
            .expect("user data should load");

        assert_eq!(overview.id, 1);
        assert!(overview.osu_folders.is_empty());
    }

    #[tokio::test]
    async fn registering_a_lazer_folder_derives_its_kind_and_marker() {
        let state = empty_state().await;
        let folder = lazer_folder();

        let registered = UserDataService::new(&state)
            .register_osu_folder(folder.path().to_path_buf(), Some("Desktop".to_owned()))
            .await
            .expect("the folder should register");

        assert!(registered.was_created());
        let installation = registered.installation();
        assert_eq!(installation.kind, OsuKind::Lazer);
        assert!(installation.marker_path.ends_with("client.realm"));
        assert_eq!(installation.label.as_deref(), Some("Desktop"));
        assert!(installation.enabled);
    }

    #[tokio::test]
    async fn registering_the_same_folder_twice_reports_the_stored_one() {
        let state = empty_state().await;
        let folder = lazer_folder();
        let service = UserDataService::new(&state);

        service
            .register_osu_folder(folder.path().to_path_buf(), None)
            .await
            .expect("the folder should register");
        let again = service
            .register_osu_folder(folder.path().to_path_buf(), None)
            .await
            .expect("the folder should resolve");

        assert!(!again.was_created());
        assert_eq!(
            service
                .osu_folders()
                .await
                .expect("folders should load")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn a_relative_path_is_rejected_without_touching_the_filesystem() {
        let state = empty_state().await;

        let error = UserDataService::new(&state)
            .register_osu_folder(PathBuf::from("osu"), None)
            .await
            .expect_err("a relative path should be rejected");

        assert!(matches!(error, RegisterFolderError::RelativePath(_)));
    }

    #[tokio::test]
    async fn a_folder_without_an_osu_marker_is_rejected() {
        let state = empty_state().await;
        let empty = tempfile::TempDir::new().expect("temp dir");

        let error = UserDataService::new(&state)
            .register_osu_folder(empty.path().to_path_buf(), None)
            .await
            .expect_err("a folder with no marker should be rejected");

        assert!(matches!(error, RegisterFolderError::NotAnOsuFolder(_)));
    }

    #[tokio::test]
    async fn a_folder_holding_two_installations_is_reported_as_ambiguous() {
        let state = empty_state().await;
        let folder = lazer_folder();
        stable_folder(&folder);

        let error = UserDataService::new(&state)
            .register_osu_folder(folder.path().to_path_buf(), None)
            .await
            .expect_err("two installations under one root should be rejected");

        let RegisterFolderError::Ambiguous { found, .. } = error else {
            panic!("expected an ambiguous folder error");
        };
        assert_eq!(found.len(), 2);
    }

    #[tokio::test]
    async fn updating_leaves_omitted_fields_alone() {
        let state = empty_state().await;
        let folder = lazer_folder();
        let service = UserDataService::new(&state);
        let id = service
            .register_osu_folder(folder.path().to_path_buf(), Some("Desktop".to_owned()))
            .await
            .expect("the folder should register")
            .into_installation()
            .id;

        let updated = service
            .update_osu_folder(
                id,
                FolderChanges {
                    enabled: Some(false),
                    ..FolderChanges::default()
                },
            )
            .await
            .expect("the folder should update")
            .expect("the folder should exist");

        assert!(!updated.enabled);
        assert_eq!(updated.label.as_deref(), Some("Desktop"));
    }

    #[tokio::test]
    async fn removing_a_folder_reports_whether_it_existed() {
        let state = empty_state().await;
        let folder = lazer_folder();
        let service = UserDataService::new(&state);
        let id = service
            .register_osu_folder(folder.path().to_path_buf(), None)
            .await
            .expect("the folder should register")
            .into_installation()
            .id;

        assert!(
            service
                .remove_osu_folder(id)
                .await
                .expect("the folder should be removed")
        );
        assert!(
            !service
                .remove_osu_folder(id)
                .await
                .expect("removal should run")
        );
        assert!(
            service
                .osu_folders()
                .await
                .expect("folders should load")
                .is_empty()
        );
    }
}
