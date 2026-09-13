pub mod audio_source;
pub mod beatmap;
pub mod beatmap_metadata;
pub mod beatmap_set;
pub mod osu_installation;
pub mod user_data;

pub use audio_source::AudioSourceRepository;
pub use beatmap::BeatmapRepository;
pub use beatmap_metadata::{BeatmapMetadataRepository, metadata_hash};
pub use beatmap_set::BeatmapSetRepository;
pub use osu_installation::{OsuInstallationRepository, RegisteredInstallation};
pub use user_data::UserDataRepository;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub beatmap_sets: usize,
    pub beatmaps: usize,
    pub audio_sources: usize,
}
