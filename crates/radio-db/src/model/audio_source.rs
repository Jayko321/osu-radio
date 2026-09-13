use std::{error::Error, fmt};

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
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Local(_) => LOCAL,
            Self::Copied(_) => COPIED,
            Self::Online(_) => ONLINE,
        }
    }

    #[must_use]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSource {
    pub id: i32,
    pub s_type: SourceType,
}
