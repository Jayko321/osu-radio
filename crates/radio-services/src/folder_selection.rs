//! Read-only previews and atomic registration/import for folder selection.
use crate::{OsuInstallationService, RegisterFolderError, RegisteredInstallation, UserDataService};
use radio_core::{
    OsuKind, OsuMarker,
    import_types::{ImportedBeatmapSet, ImportedSnapshot},
};
pub use radio_scanner::discovery::{DiscoveryDepth, DiscoveryOptions};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct FolderCandidate {
    pub marker: OsuMarker,
    pub registered_id: Option<i32>,
}

/// Owns the scanner stream: dropping the HTTP body also cancels its walk.
pub struct FolderDiscovery {
    discovery: radio_scanner::discovery::Discovery,
    registered: HashMap<PathBuf, i32>,
}
impl FolderDiscovery {
    pub async fn next(&mut self) -> Option<FolderCandidate> {
        let mut marker = self.discovery.next().await?;
        marker.marker_path = resolved_path(&marker.marker_path);
        marker.root_path = marker.marker_path.parent()?.to_path_buf();
        Some(FolderCandidate {
            registered_id: self.registered.get(&marker.marker_path).copied(),
            marker,
        })
    }
}

fn resolved_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl OsuInstallationService<'_> {
    pub async fn discover_folders(
        &self,
        options: DiscoveryOptions,
    ) -> anyhow::Result<FolderDiscovery> {
        let registered = self
            .all()
            .await?
            .into_iter()
            .map(|folder| (resolved_path(&folder.marker_path), folder.id))
            .collect();
        Ok(FolderDiscovery {
            discovery: radio_scanner::discovery::discover(options),
            registered,
        })
    }

    pub async fn folder_metadata(
        &self,
        marker_path: PathBuf,
    ) -> Result<usize, RegisterFolderError> {
        let marker = resolve_marker(&marker_path)?;
        let sets = radio_scanner::get_beatmap_sets(marker)
            .await
            .map_err(RegisterFolderError::Failed)?;
        Ok(sets.iter().map(|set| set.beatmaps.len()).sum())
    }

    pub async fn import_folder(
        &self,
        marker_path: PathBuf,
    ) -> Result<RegisteredInstallation, RegisterFolderError> {
        let marker = resolve_marker(&marker_path)?;
        // An already registered source must not be read or have its snapshot replaced.
        if let Some(folder) = self
            .all()
            .await
            .map_err(RegisterFolderError::Failed)?
            .into_iter()
            .find(|folder| resolved_path(&folder.marker_path) == marker.marker_path)
        {
            return Ok(RegisteredInstallation::AlreadyRegistered(folder));
        }
        let snapshot = radio_scanner::import_snapshot(marker.clone())
            .await
            .map_err(RegisterFolderError::Failed)?;
        self.register_imported_snapshot(&marker, &snapshot)
            .await
            .map_err(RegisterFolderError::Failed)
    }

    /// Source reading precedes this transaction; registration and its snapshot commit together.
    pub async fn register_snapshot(
        &self,
        marker: &OsuMarker,
        imported: &[ImportedBeatmapSet],
    ) -> anyhow::Result<RegisteredInstallation> {
        self.register_imported_snapshot(
            marker,
            &ImportedSnapshot {
                beatmap_sets: imported.to_vec(),
                collections: Vec::new(),
            },
        )
        .await
    }

    pub async fn register_imported_snapshot(
        &self,
        marker: &OsuMarker,
        imported: &ImportedSnapshot,
    ) -> anyhow::Result<RegisteredInstallation> {
        let transaction = self.database.begin().await?;
        UserDataService::lock(transaction.user_data()).await?;
        // Repeat identity matching under the writer lock for concurrent imports and legacy aliases.
        let existing = transaction
            .osu_installations()
            .all()
            .await?
            .into_iter()
            .find(|folder| {
                resolved_path(&folder.marker_path) == resolved_path(&marker.marker_path)
            });
        let registered = if let Some(folder) = existing {
            RegisteredInstallation::AlreadyRegistered(folder)
        } else {
            transaction
                .osu_installations()
                .register(marker, None)
                .await?
        };
        if registered.was_created() {
            Self::replace_imported_snapshot_in(
                &transaction,
                registered.installation().id,
                imported,
            )
            .await?;
        }
        let folder = transaction
            .osu_installations()
            .get(registered.installation().id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Imported installation disappeared"))?;
        let result = if registered.was_created() {
            RegisteredInstallation::Created(folder)
        } else {
            RegisteredInstallation::AlreadyRegistered(folder)
        };
        transaction.commit().await?;
        Ok(result)
    }
    /// Read the complete source first; recheck its persisted identity before any replacement.
    pub async fn reimport_folder(
        &self,
        id: i32,
    ) -> Result<Option<crate::model::OsuInstallation>, RegisterFolderError> {
        let Some(folder) = self.get(id).await.map_err(RegisterFolderError::Failed)? else {
            return Ok(None);
        };
        let marker = resolve_marker(&folder.marker_path)?;
        let snapshot = radio_scanner::import_snapshot(marker.clone())
            .await
            .map_err(RegisterFolderError::Failed)?;
        self.reimport_snapshot(id, &marker, &snapshot)
            .await
            .map_err(RegisterFolderError::Failed)
    }

    pub(super) async fn reimport_snapshot(
        &self,
        id: i32,
        marker: &OsuMarker,
        imported: &ImportedSnapshot,
    ) -> anyhow::Result<Option<crate::model::OsuInstallation>> {
        let transaction = self.database.begin().await?;
        UserDataService::lock(transaction.user_data()).await?;
        let Some(folder) = transaction.osu_installations().get(id).await? else {
            return Ok(None);
        };
        anyhow::ensure!(
            folder.kind == marker.kind && resolved_path(&folder.marker_path) == marker.marker_path,
            "Registered source changed during import"
        );
        Self::replace_imported_snapshot_in(&transaction, id, imported).await?;
        let folder = transaction.osu_installations().get(id).await?;
        transaction.commit().await?;
        Ok(folder)
    }
}

fn resolve_marker(path: &Path) -> Result<OsuMarker, RegisterFolderError> {
    if !path.is_absolute() {
        return Err(RegisterFolderError::RelativePath(path.to_path_buf()));
    }
    let marker_path = std::fs::canonicalize(path)
        .map_err(|_| RegisterFolderError::NotAnOsuFolder(path.to_path_buf()))?;
    let kind = match marker_path.file_name().and_then(|name| name.to_str()) {
        Some("osu!.db") => OsuKind::Stable,
        Some("client.realm") => OsuKind::Lazer,
        _ => return Err(RegisterFolderError::NotAnOsuFolder(path.to_path_buf())),
    };
    if !marker_path.is_file() {
        return Err(RegisterFolderError::NotAnOsuFolder(path.to_path_buf()));
    }
    let root_path = marker_path
        .parent()
        .ok_or_else(|| RegisterFolderError::NotAnOsuFolder(path.to_path_buf()))?
        .to_path_buf();
    Ok(OsuMarker {
        kind,
        root_path,
        marker_path,
    })
}
