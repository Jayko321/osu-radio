use crate::schema::beatmaps;

#[derive(Debug, Clone, diesel::Identifiable, diesel::Queryable)]
#[diesel(table_name = beatmaps)]
pub struct Beatmap {
    pub id: i32,
    pub source: String,
    pub difficulty_name: Option<String>,
    pub bpm: Option<f64>,
    pub hash: Option<String>,
    pub beatmap_set_id: Option<i32>,
    pub metadata_id: Option<i32>,
}

#[derive(Debug, Clone, Copy, diesel::Insertable)]
#[diesel(table_name = beatmaps)]
pub struct NewBeatmap<'a> {
    pub source: &'a str,
    pub difficulty_name: Option<&'a str>,
    pub bpm: Option<f64>,
    pub hash: Option<&'a str>,
    pub beatmap_set_id: Option<i32>,
    pub metadata_id: Option<i32>,
}
