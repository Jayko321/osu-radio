use super::SourceType;
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackMode {
    Stopped,
    Paused,
    Playing,
}

/// A positive BIGINT. Frontends must transport this as a decimal string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueueEntryId(i64);
impl QueueEntryId {
    pub fn new(value: i64) -> Result<Self> {
        ensure!(value > 0, "Queue entry ID must be positive");
        Ok(Self(value))
    }
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}
impl std::fmt::Display for QueueEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueAvailability {
    Checking,
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueuePlaylistSource {
    pub installation_kind: String,
    pub hash_kind: String,
    pub beatmap_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntry {
    pub id: QueueEntryId,
    pub position: usize,
    pub audio_source: Option<SourceType>,
    pub playlist_source: Option<QueuePlaylistSource>,
    // Advisory references: source removal must never delete queue entries or their snapshots.
    pub audio_source_id: Option<i32>,
    pub playlist_item_id: Option<i32>,
    pub migration_audio_source_id: Option<i32>,
    pub migration_playlist_item_id: Option<i32>,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub difficulty_name: Option<String>,
    pub beatmap_id: Option<i32>,
    pub beatmap_set_id: Option<i32>,
    pub cover_beatmap_id: Option<i32>,
    pub availability: QueueAvailability,
    pub check_generation: u64,
    /// Original order key; present on every entry while shuffle is on, absent otherwise.
    pub shuffle_order: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueLaunchSource {
    pub audio_source_id: i32,
    pub source: SourceType,
    pub check_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueState {
    pub entries: Vec<QueueEntry>,
    pub current_entry_id: Option<QueueEntryId>,
    /// Current entry position, or the boundary separating past from future.
    pub history_len: usize,
    pub mode: PlaybackMode,
    pub waiting_for_check: bool,
    pub launch_source: Option<QueueLaunchSource>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// The current launch is a Repeat One restart and must not record listening history.
    pub repeat_launch: bool,
    pub revision: u64,
    pub playback_token: u64,
    pub last_entry_id: i64,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            current_entry_id: None,
            history_len: 0,
            mode: PlaybackMode::Stopped,
            waiting_for_check: false,
            launch_source: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            repeat_launch: false,
            revision: 0,
            playback_token: 0,
            last_entry_id: 0,
        }
    }
}
impl QueueState {
    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        self.current_entry_id
            .and_then(|id| self.entries.iter().position(|e| e.id == id))
    }
    #[must_use]
    pub fn current_entry(&self) -> Option<&QueueEntry> {
        self.current_entry_id
            .and_then(|id| self.entries.iter().find(|e| e.id == id))
    }
    #[must_use]
    pub fn audio_source_ids(&self) -> Vec<i32> {
        self.entries
            .iter()
            .filter_map(|e| e.audio_source_id.or(e.migration_audio_source_id))
            .collect()
    }
    #[must_use]
    pub fn playlist_item_ids(&self) -> Vec<Option<i32>> {
        self.entries
            .iter()
            .map(|e| e.playlist_item_id.or(e.migration_playlist_item_id))
            .collect()
    }
    pub fn allocate_id(&mut self) -> Result<QueueEntryId> {
        let next = self
            .last_entry_id
            .checked_add(1)
            .context("Queue entry IDs exhausted")?;
        let id = QueueEntryId::new(next)?;
        self.last_entry_id = next;
        Ok(id)
    }
    pub fn reindex(&mut self) {
        for (position, entry) in self.entries.iter_mut().enumerate() {
            entry.position = position;
        }
        if let Some(index) = self.current_index() {
            self.history_len = index;
        }
    }
    pub fn validate(&self) -> Result<()> {
        if let Some(source) = &self.launch_source {
            i64::try_from(source.check_generation)?;
            ensure!(
                self.mode != PlaybackMode::Stopped
                    && self.current_entry_id.is_some()
                    && !self.waiting_for_check,
                "Launch source requires an assigned current launch"
            );
        }
        let mut ids = std::collections::HashSet::new();
        ensure!(self.last_entry_id >= 0, "Negative queue ID counter");
        i64::try_from(self.entries.len())?;
        i64::try_from(self.history_len)?;
        i64::try_from(self.revision)?;
        i64::try_from(self.playback_token)?;
        for (position, entry) in self.entries.iter().enumerate() {
            ensure!(entry.position == position, "Queue order must be dense");
            ensure!(ids.insert(entry.id), "Duplicate queue entry ID");
            ensure!(
                entry.id.get() <= self.last_entry_id,
                "Queue entry exceeds issued ID counter"
            );
            i64::try_from(entry.position)?;
            i64::try_from(entry.check_generation)?;
            ensure!(
                entry.shuffle_order.is_some() == self.shuffle,
                "Shuffle order keys must exist exactly while shuffle is on"
            );
            if let Some(source) = &entry.playlist_source {
                ensure!(
                    matches!(source.installation_kind.as_str(), "stable" | "lazer")
                        && matches!(source.hash_kind.as_str(), "source" | "md5"),
                    "Invalid queue playlist source key"
                );
            }
        }
        ensure!(
            self.history_len <= self.entries.len(),
            "Invalid queue history boundary"
        );
        if let Some(id) = self.current_entry_id {
            let current = self
                .current_entry()
                .context("Current queue entry is missing")?;
            ensure!(
                current.id == id && current.position == self.history_len,
                "Current queue entry must match history boundary"
            );
        }
        ensure!(
            self.mode == PlaybackMode::Stopped || self.current_entry_id.is_some(),
            "Active playback requires a current queue entry"
        );
        ensure!(
            !self.waiting_for_check
                || (self.mode != PlaybackMode::Stopped
                    && self
                        .current_entry()
                        .is_some_and(|e| e.availability == QueueAvailability::Checking)),
            "Waiting requires an active current entry in checking"
        );
        Ok(())
    }
}
