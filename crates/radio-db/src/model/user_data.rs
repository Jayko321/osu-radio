pub const USER_DATA_ID: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserData {
    pub id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSettings {
    pub individual_volume_enabled: bool,
    pub global_volume_percent: u8,
}
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            individual_volume_enabled: false,
            global_volume_percent: 100,
        }
    }
}
