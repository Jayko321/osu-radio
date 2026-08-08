diesel::table! {
    beatmap_metadata (id) {
        id -> Integer,
        title -> Nullable<Text>,
        title_unicode -> Nullable<Text>,
        artist -> Nullable<Text>,
        artist_unicode -> Nullable<Text>,
        author_online_id -> Nullable<Integer>,
        author_username -> Nullable<Text>,
        author_country_code -> Nullable<Text>,
        source -> Nullable<Text>,
        tags -> Nullable<Text>,
        user_tags -> Nullable<Text>,
        preview_time -> Nullable<Integer>,
        audio_source_id -> Nullable<Integer>,
        background_file -> Nullable<Text>,
    }
}
