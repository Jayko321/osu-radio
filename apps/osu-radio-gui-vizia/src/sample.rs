use crate::assets;

pub struct Track {
    pub title: &'static str,
    pub artist: &'static str,
    pub duration: &'static str,
    pub cover: &'static str,
    pub tint: &'static str,
}

impl Track {
    pub fn meta(&self) -> String {
        format!("{} // {}", self.artist, self.duration)
    }
}

pub const TRACKS: &[Track] = &[
    Track {
        title: "Karakara",
        artist: "kessoku band",
        duration: "04:21",
        cover: "url(\"cover-karakara\")",
        tint: "tint-navy",
    },
    Track {
        title: "Bling-Bang-Bang-Born (TV Size)",
        artist: "Creepy Nuts",
        duration: "04:21",
        cover: "url(\"cover-bbbb\")",
        tint: "tint-olive",
    },
    Track {
        title: "Rabbit hole feat. Hatsune Miku",
        artist: "kessoku band",
        duration: "04:21",
        cover: "url(\"cover-rabbit\")",
        tint: "tint-plum",
    },
    Track {
        title: "Guitar to Kodoku to Aoi Hoshi",
        artist: "KikouHana",
        duration: "04:21",
        cover: "url(\"cover-alice\")",
        tint: "tint-maroon",
    },
    Track {
        title: "Seishun Complex",
        artist: "kessoku band",
        duration: "03:44",
        cover: "url(\"cover-karakara\")",
        tint: "tint-navy",
    },
    Track {
        title: "Otherside",
        artist: "Creepy Nuts",
        duration: "03:12",
        cover: "url(\"cover-bbbb\")",
        tint: "tint-olive",
    },
    Track {
        title: "Hitoribocchi Tokyo",
        artist: "KikouHana",
        duration: "04:58",
        cover: "url(\"cover-rabbit\")",
        tint: "tint-plum",
    },
    Track {
        title: "Distortion!!",
        artist: "kessoku band",
        duration: "02:51",
        cover: "url(\"cover-alice\")",
        tint: "tint-maroon",
    },
];

pub const PLAYING: usize = 3;

pub const ELAPSED: &str = "01:22";

pub const FILTERS: [&str; 3] = ["Title", "All musics", "Tags"];

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
