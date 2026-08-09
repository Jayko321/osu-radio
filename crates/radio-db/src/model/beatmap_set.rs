use crate::schema::beatmap_sets;

#[derive(Debug, Clone, diesel::Identifiable, diesel::Queryable)]
#[diesel(table_name = beatmap_sets)]
pub struct BeatmapSet {
    pub id: i32,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub installation_id: Option<i32>,
}

#[derive(Debug, Clone, Copy, diesel::Insertable)]
#[diesel(table_name = beatmap_sets)]
pub struct NewBeatmapSet<'a> {
    pub online_id: Option<i32>,
    pub hash: Option<&'a str>,
    pub installation_id: Option<i32>,
}
