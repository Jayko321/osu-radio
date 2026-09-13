#[derive(Debug, Clone, PartialEq)]
pub struct Beatmap {
    pub id: i32,
    pub difficulty_name: Option<String>,
    pub bpm: Option<f64>,
    pub hash: Option<String>,
    pub beatmap_set_id: i32,
    pub metadata_hash: Option<String>,
    pub audio_source_id: Option<i32>,
}
