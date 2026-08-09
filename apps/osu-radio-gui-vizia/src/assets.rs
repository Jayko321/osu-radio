use vizia::prelude::*;

pub const SEARCH: &[u8] = include_bytes!("../assets/icons/search-line.svg");
pub const CHEVRON: &[u8] = include_bytes!("../assets/icons/arrow-down-s-line.svg");
pub const PLAY: &[u8] = include_bytes!("../assets/icons/play-fill.svg");
pub const SKIP_FORWARD: &[u8] = include_bytes!("../assets/icons/skip-forward-mini-fill.svg");
pub const SKIP_BACK: &[u8] = include_bytes!("../assets/icons/skip-back-mini-fill.svg");
pub const SHUFFLE: &[u8] = include_bytes!("../assets/icons/shuffle-line.svg");
pub const REPEAT: &[u8] = include_bytes!("../assets/icons/repeat-2-line.svg");
pub const VOLUME: &[u8] = include_bytes!("../assets/icons/volume-up-fill.svg");
pub const ADD_CIRCLE: &[u8] = include_bytes!("../assets/icons/add-circle-line.svg");
pub const STACK: &[u8] = include_bytes!("../assets/icons/stack-line.svg");
pub const MUSIC: &[u8] = include_bytes!("../assets/icons/music-fill.svg");
pub const SETTINGS: &[u8] = include_bytes!("../assets/icons/settings-4-line.svg");
pub const PENCIL: &[u8] = include_bytes!("../assets/icons/pencil-line.svg");
pub const MINIMIZE: &[u8] = include_bytes!("../assets/icons/subtract-line.svg");
pub const MAXIMIZE: &[u8] = include_bytes!("../assets/icons/checkbox-blank-line.svg");
pub const RESTORE: &[u8] = include_bytes!("../assets/icons/file-copy-line.svg");
pub const CLOSE: &[u8] = include_bytes!("../assets/icons/close-line.svg");

const NUNITO: &[u8] = include_bytes!("../assets/fonts/Nunito-Variable.ttf");

const COVERS: [(&str, &[u8]); 4] = [
    (
        "cover-karakara",
        include_bytes!("../assets/covers/karakara.jpg"),
    ),
    ("cover-bbbb", include_bytes!("../assets/covers/bbbb.jpg")),
    (
        "cover-rabbit",
        include_bytes!("../assets/covers/rabbit.jpg"),
    ),
    ("cover-alice", include_bytes!("../assets/covers/alice.jpg")),
];

const COVER_IMAGES: [&str; 4] = [
    "url(\"cover-karakara\")",
    "url(\"cover-bbbb\")",
    "url(\"cover-rabbit\")",
    "url(\"cover-alice\")",
];

const TINTS: [&str; 4] = ["tint-navy", "tint-olive", "tint-plum", "tint-maroon"];

/// Artwork is keyed on a track's position until beatmap sets carry covers of their own, so the
/// bundled four repeat down the list rather than living on the track itself.
pub fn cover_image(index: usize) -> &'static str {
    pick(&COVER_IMAGES, index)
}

pub fn tint(index: usize) -> &'static str {
    pick(&TINTS, index)
}

fn pick(options: &[&'static str], index: usize) -> &'static str {
    index
        .checked_rem(options.len())
        .and_then(|position| options.get(position))
        .copied()
        .unwrap_or_default()
}

pub fn register(cx: &mut Context) {
    cx.add_font_mem(NUNITO);

    for (name, data) in COVERS {
        cx.load_image(name, data, ImageRetentionPolicy::Forever);
    }
}
