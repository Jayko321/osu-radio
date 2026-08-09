use std::{sync::LazyLock, time::Duration};

use osu_radio_client::Track;

use crate::assets;

pub static TRACKS: LazyLock<Vec<Track>> = LazyLock::new(|| {
    vec![
        Track::new("Karakara", "kessoku band", Duration::from_secs(261)),
        Track::new(
            "Bling-Bang-Bang-Born (TV Size)",
            "Creepy Nuts",
            Duration::from_secs(261),
        ),
        Track::new(
            "Rabbit hole feat. Hatsune Miku",
            "kessoku band",
            Duration::from_secs(261),
        ),
        Track::new(
            "Guitar to Kodoku to Aoi Hoshi",
            "KikouHana",
            Duration::from_secs(261),
        ),
        Track::new("Seishun Complex", "kessoku band", Duration::from_secs(224)),
        Track::new("Otherside", "Creepy Nuts", Duration::from_secs(192)),
        Track::new("Hitoribocchi Tokyo", "KikouHana", Duration::from_secs(298)),
        Track::new("Distortion!!", "kessoku band", Duration::from_secs(171)),
    ]
});

pub const PLAYING: usize = 3;

pub const ELAPSED: &str = "01:22";

pub const FILTERS: &[&str] = &["Title", "All musics", "Tags"];

pub struct Field {
    pub label: &'static str,
    pub value: &'static str,
    pub pickable: bool,
}

pub struct Section {
    pub title: &'static str,
    pub glyph: &'static [u8],
    pub fields: &'static [Field],
}

pub const SECTIONS: &[Section] = &[
    Section {
        title: "General",
        glyph: assets::PENCIL,
        fields: &[
            Field {
                label: "Songs folder",
                value: "some/path/to/idk/?",
                pickable: true,
            },
            Field {
                label: "Other setting",
                value: "Something? dunno",
                pickable: false,
            },
        ],
    },
    Section {
        title: "Audio",
        glyph: assets::VOLUME,
        fields: &[Field {
            label: "Output device",
            value: "Speakers 212434",
            pickable: true,
        }],
    },
];
