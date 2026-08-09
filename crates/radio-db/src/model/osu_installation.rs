#![allow(clippy::ref_option_ref)]

use diesel::backend::Backend;
use diesel::deserialize::{self, FromStaticSqlRow, Queryable};
use diesel::expression::Selectable;
use diesel::sql_types::{Bool, Integer, Nullable, Text};
use radio_core::OsuKind;

use crate::schema::osu_installations;

type InstallationSqlRow = (
    Integer,
    Integer,
    Text,
    Text,
    Text,
    Nullable<Text>,
    Bool,
    Nullable<Text>,
);

type InstallationRow = (
    i32,
    i32,
    String,
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
);

#[derive(Debug, Clone, PartialEq, Eq, diesel::Identifiable)]
#[diesel(table_name = osu_installations)]
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

impl<DB> Queryable<InstallationSqlRow, DB> for OsuInstallation
where
    DB: Backend,
    InstallationRow: FromStaticSqlRow<InstallationSqlRow, DB>,
{
    type Row = InstallationRow;

    fn build(
        (id, user_data_id, kind, root_path, marker_path, label, enabled, last_scanned_at): Self::Row,
    ) -> deserialize::Result<Self> {
        Ok(Self {
            id,
            user_data_id,
            kind: kind.parse()?,
            root_path,
            marker_path,
            label,
            enabled,
            last_scanned_at,
        })
    }
}

impl<DB> Selectable<DB> for OsuInstallation
where
    DB: Backend,
{
    type SelectExpression = (
        osu_installations::id,
        osu_installations::user_data_id,
        osu_installations::kind,
        osu_installations::root_path,
        osu_installations::marker_path,
        osu_installations::label,
        osu_installations::enabled,
        osu_installations::last_scanned_at,
    );

    fn construct_selection() -> Self::SelectExpression {
        (
            osu_installations::id,
            osu_installations::user_data_id,
            osu_installations::kind,
            osu_installations::root_path,
            osu_installations::marker_path,
            osu_installations::label,
            osu_installations::enabled,
            osu_installations::last_scanned_at,
        )
    }
}

#[derive(Debug, Clone, Copy, diesel::Insertable)]
#[diesel(table_name = osu_installations)]
pub struct NewOsuInstallation<'a> {
    pub user_data_id: i32,
    pub kind: &'a str,
    pub root_path: &'a str,
    pub marker_path: &'a str,
    pub label: Option<&'a str>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Default, diesel::AsChangeset)]
#[diesel(table_name = osu_installations)]
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
