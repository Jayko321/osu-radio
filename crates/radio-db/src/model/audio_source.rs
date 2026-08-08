use std::error::Error;
use std::fmt;

use diesel::backend::Backend;
use diesel::deserialize::{self, FromStaticSqlRow, Queryable};
use diesel::expression::Selectable;
use diesel::sql_types::{Integer, Text};

use crate::schema::audio_sources;

const LOCAL: &str = "local";
const COPIED: &str = "copied";
const ONLINE: &str = "online";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceType {
    Local(String),
    Copied(String),
    Online(String),
}

impl SourceType {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Local(_) => LOCAL,
            Self::Copied(_) => COPIED,
            Self::Online(_) => ONLINE,
        }
    }

    pub fn location(&self) -> &str {
        match self {
            Self::Local(location) | Self::Copied(location) | Self::Online(location) => location,
        }
    }

    pub fn from_parts(kind: &str, location: String) -> Result<Self, UnknownSourceKind> {
        match kind {
            LOCAL => Ok(Self::Local(location)),
            COPIED => Ok(Self::Copied(location)),
            ONLINE => Ok(Self::Online(location)),
            other => Err(UnknownSourceKind(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSourceKind(pub String);

impl fmt::Display for UnknownSourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown audio source kind `{}`", self.0)
    }
}

impl Error for UnknownSourceKind {}

#[derive(Debug, Clone, PartialEq, Eq, diesel::Identifiable)]
#[diesel(table_name = audio_sources)]
pub struct AudioSource {
    pub id: i32,
    pub s_type: SourceType,
}

impl<DB> Queryable<(Integer, Text, Text), DB> for AudioSource
where
    DB: Backend,
    (i32, String, String): FromStaticSqlRow<(Integer, Text, Text), DB>,
{
    type Row = (i32, String, String);

    fn build((id, kind, location): Self::Row) -> deserialize::Result<Self> {
        Ok(Self {
            id,
            s_type: SourceType::from_parts(&kind, location)?,
        })
    }
}

impl<DB> Selectable<DB> for AudioSource
where
    DB: Backend,
{
    type SelectExpression = (
        audio_sources::id,
        audio_sources::kind,
        audio_sources::location,
    );

    fn construct_selection() -> Self::SelectExpression {
        (
            audio_sources::id,
            audio_sources::kind,
            audio_sources::location,
        )
    }
}

#[derive(Debug, Clone, Copy, diesel::Insertable)]
#[diesel(table_name = audio_sources)]
pub struct NewAudioSource<'a> {
    pub kind: &'a str,
    pub location: &'a str,
}

impl<'a> From<&'a SourceType> for NewAudioSource<'a> {
    fn from(source: &'a SourceType) -> Self {
        Self {
            kind: source.kind(),
            location: source.location(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NewAudioSource, SourceType, UnknownSourceKind};

    #[test]
    fn source_type_round_trips_through_its_column_pair() {
        for source in [
            SourceType::Local("/osu/files/a/ab/abc".to_owned()),
            SourceType::Copied("/app/audio/abc.mp3".to_owned()),
            SourceType::Online("https://example.invalid/abc.mp3".to_owned()),
        ] {
            let columns = NewAudioSource::from(&source);
            let rebuilt = SourceType::from_parts(columns.kind, columns.location.to_owned())
                .expect("known kind should parse");

            assert_eq!(rebuilt, source);
        }
    }

    #[test]
    fn unknown_source_kind_is_rejected() {
        let error = SourceType::from_parts("streamed", "location".to_owned())
            .expect_err("unknown kind should not parse");

        assert_eq!(error, UnknownSourceKind("streamed".to_owned()));
    }
}
