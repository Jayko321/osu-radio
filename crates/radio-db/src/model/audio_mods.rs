#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AudioModsMode {
    #[default]
    None,
    Dt,
    Ht,
}

impl AudioModsMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Dt => "dt",
            Self::Ht => "ht",
        }
    }
}

impl std::str::FromStr for AudioModsMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "dt" => Ok(Self::Dt),
            "ht" => Ok(Self::Ht),
            _ => anyhow::bail!("Unknown audio mods mode: {value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioModsProfile {
    pub mode: AudioModsMode,
    pub speed_percent: u16,
    pub adjust_pitch: bool,
}

impl Default for AudioModsProfile {
    fn default() -> Self {
        Self {
            mode: AudioModsMode::None,
            speed_percent: 100,
            adjust_pitch: false,
        }
    }
}

impl AudioModsProfile {
    #[must_use]
    pub const fn is_valid(self) -> bool {
        match self.mode {
            AudioModsMode::None => self.speed_percent == 100 && !self.adjust_pitch,
            AudioModsMode::Dt => self.speed_percent >= 101 && self.speed_percent <= 200,
            AudioModsMode::Ht => self.speed_percent >= 50 && self.speed_percent <= 99,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AudioModsSettings {
    pub individual_mods_enabled: bool,
    pub global_profile: AudioModsProfile,
}
