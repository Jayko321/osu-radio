pub mod import_types;

use std::{error::Error, fmt, path::PathBuf, str::FromStr};

const STABLE: &str = "stable";
const LAZER: &str = "lazer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsuKind {
    Stable,
    Lazer,
}

impl OsuKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => STABLE,
            Self::Lazer => LAZER,
        }
    }
}

impl FromStr for OsuKind {
    type Err = UnknownOsuKind;

    fn from_str(kind: &str) -> Result<Self, Self::Err> {
        match kind {
            STABLE => Ok(Self::Stable),
            LAZER => Ok(Self::Lazer),
            other => Err(UnknownOsuKind(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownOsuKind(pub String);

impl fmt::Display for UnknownOsuKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown osu! source kind `{}`", self.0)
    }
}

impl Error for UnknownOsuKind {}

#[derive(Debug, Clone)]
pub struct OsuMarker {
    pub kind: OsuKind,
    pub marker_path: PathBuf,
    pub root_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::{OsuKind, UnknownOsuKind};

    #[test]
    fn osu_kind_round_trips_through_its_stored_string() {
        for kind in [OsuKind::Stable, OsuKind::Lazer] {
            assert_eq!(kind.as_str().parse(), Ok(kind));
        }
    }

    #[test]
    fn unknown_osu_kind_is_rejected() {
        assert_eq!(
            "tachyon".parse::<OsuKind>(),
            Err(UnknownOsuKind("tachyon".to_owned()))
        );
    }
}
