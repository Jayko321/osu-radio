#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackMode {
    Stopped,
    Paused,
    Playing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueState {
    pub audio_source_ids: Vec<i32>,
    /// `None` for an empty queue; the queue length marks exhaustion.
    pub current_index: Option<usize>,
    pub mode: PlaybackMode,
    pub revision: u64,
    pub playback_token: u64,
}
