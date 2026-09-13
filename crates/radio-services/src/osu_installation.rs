use crate::{
    AudioSourceService, BeatmapMetadataService, BeatmapService, BeatmapSetService, ImportSummary,
    RegisteredInstallation, UserDataService,
    model::{OsuInstallation, OsuInstallationChanges, SourceType},
};
use anyhow::{Context, Result, ensure};
use radio_core::{OsuMarker, import_types::ImportedBeatmapSet};
use radio_db::Database;
use radio_scanner::discovery::{DiscoveryDepth, DiscoveryOptions, discover};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum RegisterFolderError {
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
pub struct FolderChanges {
    #[allow(clippy::option_option)]
    pub label: Option<Option<String>>,
    pub enabled: Option<bool>,
}

pub struct OsuInstallationService<'a> {
    pub(crate) database: &'a Database,
}
impl OsuInstallationService<'_> {
    pub async fn all(&self) -> Result<Vec<OsuInstallation>> {
        self.database
            .osu_installations()
            .all()
            .await
            .context("Failed to load the registered osu! folders")
    }
    pub async fn get(&self, id: i32) -> Result<Option<OsuInstallation>> {
        self.database.osu_installations().get(id).await
    }
    /// Registers a scanner-resolved marker without repeating discovery.
    pub async fn register(
        &self,
        marker: &OsuMarker,
        label: Option<&str>,
    ) -> Result<RegisteredInstallation> {
        self.database
            .osu_installations()
            .register(marker, label)
            .await
            .context("Failed to register the osu! folder")
    }
    pub async fn register_folder(
        &self,
        path: PathBuf,
        label: Option<String>,
    ) -> Result<RegisteredInstallation, RegisterFolderError> {
        let marker = resolve_osu_folder(&path).await?;
        self.register(&marker, label.as_deref())
            .await
            .map_err(RegisterFolderError::Failed)
    }
    pub async fn update(&self, id: i32, changes: FolderChanges) -> Result<Option<OsuInstallation>> {
        let transaction = self.database.begin().await?;
        let result = transaction
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
            .context("Failed to update the registered osu! folder")?;
        transaction.commit().await?;
        Ok(result)
    }
    pub async fn delete(&self, id: i32) -> Result<bool> {
        let transaction = self.database.begin().await?;
        UserDataService::lock(transaction.user_data()).await?;
        let deleted = transaction
            .osu_installations()
            .delete(id)
            .await
            .context("Failed to remove the registered osu! folder")?;
        BeatmapMetadataService {
            repository: transaction.beatmap_metadata(),
        }
        .cleanup()
        .await?;
        AudioSourceService {
            repository: transaction.audio_sources(),
        }
        .cleanup()
        .await?;
        transaction.commit().await?;
        Ok(deleted)
    }
    /// Replaces a fully read scanner snapshot atomically. Cancellation drops and rolls back the transaction.
    pub async fn replace_snapshot(
        &self,
        id: i32,
        imported: &[ImportedBeatmapSet],
    ) -> Result<ImportSummary> {
        let transaction = self.database.begin().await?;
        let summary = Self::replace_snapshot_in(&transaction, id, imported).await?;
        transaction.commit().await?;
        Ok(summary)
    }

    pub(super) async fn replace_snapshot_in(
        transaction: &radio_db::Transaction,
        id: i32,
        imported: &[ImportedBeatmapSet],
    ) -> Result<ImportSummary> {
        UserDataService::lock(transaction.user_data()).await?;
        let installations = transaction.osu_installations();
        let installation = installations
            .get(id)
            .await?
            .context("Installation does not exist")?;
        ensure!(
            imported.iter().all(|set| set.source == installation.kind),
            "Imported source kind does not match the registered installation"
        );
        let sets = BeatmapSetService {
            repository: transaction.beatmap_sets(),
        };
        let maps = BeatmapService {
            repository: transaction.beatmaps(),
        };
        let metadata = BeatmapMetadataService {
            repository: transaction.beatmap_metadata(),
        };
        let audio = AudioSourceService {
            repository: transaction.audio_sources(),
        };
        sets.delete_for_installation(id).await?;
        let mut summary = ImportSummary {
            beatmap_sets: imported.len(),
            ..ImportSummary::default()
        };
        let mut audio_ids = HashSet::new();
        for imported_set in imported {
            let set = sets.add(id, imported_set).await?;
            for imported_beatmap in &imported_set.beatmaps {
                let metadata_hash = match &imported_beatmap.metadata {
                    Some(imported) => Some(metadata.get_or_insert(imported).await?.hash),
                    None => None,
                };
                let audio_id = match imported_set.resolved_audio_path(imported_beatmap) {
                    Some(path) => {
                        let source = SourceType::Local(path.to_string_lossy().into_owned());
                        let id = audio.get_or_insert(&source).await?.id;
                        audio_ids.insert(id);
                        Some(id)
                    }
                    None => None,
                };
                maps.add(set.id, imported_beatmap, metadata_hash, audio_id)
                    .await?;
            }
            summary.beatmaps = summary.beatmaps.saturating_add(imported_set.beatmaps.len());
        }
        metadata.cleanup().await?;
        audio.cleanup().await?;
        installations.mark_scanned(id).await?;
        summary.audio_sources = audio_ids.len();
        Ok(summary)
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

    use crate::{FolderChanges, RegisterFolderError, Services};
    use std::{fs, path::PathBuf};
    async fn empty_state() -> Services {
        let services = Services::connect(":memory:").await.unwrap();
        services.migrate().await.unwrap();
        services
    }
    fn lazer_folder() -> tempfile::TempDir {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("client.realm"), b"").unwrap();
        folder
    }
    fn stable_folder(folder: &tempfile::TempDir) {
        fs::write(folder.path().join("osu!.db"), b"").unwrap();
    }

    #[tokio::test]
    async fn a_fresh_database_has_the_settings_row_and_no_folders() {
        let state = empty_state().await;

        let overview = state
            .user_data()
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

        let registered = state
            .osu_installations()
            .register_folder(folder.path().to_path_buf(), Some("Desktop".to_owned()))
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
        let service = state.osu_installations();

        service
            .register_folder(folder.path().to_path_buf(), None)
            .await
            .expect("the folder should register");
        let again = service
            .register_folder(folder.path().to_path_buf(), None)
            .await
            .expect("the folder should resolve");

        assert!(!again.was_created());
        assert_eq!(service.all().await.expect("folders should load").len(), 1);
    }

    #[tokio::test]
    async fn a_relative_path_is_rejected_without_touching_the_filesystem() {
        let state = empty_state().await;

        let error = state
            .osu_installations()
            .register_folder(PathBuf::from("osu"), None)
            .await
            .expect_err("a relative path should be rejected");

        assert!(matches!(error, RegisterFolderError::RelativePath(_)));
    }

    #[tokio::test]
    async fn a_folder_without_an_osu_marker_is_rejected() {
        let state = empty_state().await;
        let empty = tempfile::TempDir::new().expect("temp dir");

        let error = state
            .osu_installations()
            .register_folder(empty.path().to_path_buf(), None)
            .await
            .expect_err("a folder with no marker should be rejected");

        assert!(matches!(error, RegisterFolderError::NotAnOsuFolder(_)));
    }

    #[tokio::test]
    async fn a_folder_holding_two_installations_is_reported_as_ambiguous() {
        let state = empty_state().await;
        let folder = lazer_folder();
        stable_folder(&folder);

        let error = state
            .osu_installations()
            .register_folder(folder.path().to_path_buf(), None)
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
        let service = state.osu_installations();
        let id = service
            .register_folder(folder.path().to_path_buf(), Some("Desktop".to_owned()))
            .await
            .expect("the folder should register")
            .into_installation()
            .id;

        let updated = service
            .update(
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
        let service = state.osu_installations();
        let id = service
            .register_folder(folder.path().to_path_buf(), None)
            .await
            .expect("the folder should register")
            .into_installation()
            .id;

        assert!(
            service
                .delete(id)
                .await
                .expect("the folder should be removed")
        );
        assert!(!service.delete(id).await.expect("removal should run"));
        assert!(service.all().await.expect("folders should load").is_empty());
    }
}
