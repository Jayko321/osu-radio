use radio_core::OsuKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsuInstallation {
    pub id: i32,
    pub user_data_id: i32,
    pub kind: OsuKind,
    pub root_path: String,
    pub marker_path: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub last_scanned_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::option_option)]
pub struct OsuInstallationChanges<'a> {
    pub label: Option<Option<&'a str>>,
    pub enabled: Option<bool>,
}

impl OsuInstallationChanges<'_> {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.label.is_none() && self.enabled.is_none()
    }
}
