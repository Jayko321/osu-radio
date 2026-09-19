#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTag {
    pub beatmap_set_id: i32,
    pub name: String,
}
