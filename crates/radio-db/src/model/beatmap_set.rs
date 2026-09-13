#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeatmapSet {
    pub id: i32,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub installation_id: i32,
}
