use std::time::Duration;

/// One entry of the songs library and the subject of the now-playing panel. Frontends differ in
/// how they draw a track, not in what one holds, so it lives here rather than in any one of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub duration: Duration,
}

impl Track {
    pub fn new(title: impl Into<String>, artist: impl Into<String>, duration: Duration) -> Self {
        Self {
            title: title.into(),
            artist: artist.into(),
            duration,
        }
    }

    #[must_use]
    pub fn duration_label(&self) -> String {
        let seconds = self.duration.as_secs();

        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }

    /// The one line a compact list shows under the title.
    #[must_use]
    pub fn meta(&self) -> String {
        format!("{} // {}", self.artist, self.duration_label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_becomes_a_padded_minutes_and_seconds_label() {
        let track = Track::new("Karakara", "kessoku band", Duration::from_secs(261));

        assert_eq!(track.duration_label(), "04:21");
        assert_eq!(track.meta(), "kessoku band // 04:21");
    }

    #[test]
    fn an_hour_long_track_keeps_counting_in_minutes() {
        let track = Track::new("Long", "Someone", Duration::from_secs(3_607));

        assert_eq!(track.duration_label(), "60:07");
    }
}
