use crate::schema::user_data;

pub const USER_DATA_ID: i32 = 1;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, diesel::Identifiable, diesel::Queryable, diesel::Selectable,
)]
#[diesel(table_name = user_data)]
pub struct UserData {
    pub id: i32,
}
