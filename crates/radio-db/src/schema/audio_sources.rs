diesel::table! {
    audio_sources (id) {
        id -> Integer,
        kind -> Text,
        location -> Text,
    }
}
