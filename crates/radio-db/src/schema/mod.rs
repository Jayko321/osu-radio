diesel::table! {
    beatmap_sets (id) {
        id -> Integer,
        online_id -> Nullable<Integer>,
        hash -> Nullable<Text>,
    }
}

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
        audio_file -> Nullable<Text>,
        background_file -> Nullable<Text>,
    }
}

diesel::table! {
    beatmaps (id) {
        id -> Integer,
        source -> Text,
        difficulty_name -> Nullable<Text>,
        bpm -> Nullable<Double>,
        hash -> Nullable<Text>,
        beatmap_set_id -> Nullable<Integer>,
        metadata_id -> Nullable<Integer>,
    }
}

diesel::joinable!(beatmaps -> beatmap_metadata (metadata_id));
diesel::joinable!(beatmaps -> beatmap_sets (beatmap_set_id));

diesel::allow_tables_to_appear_in_same_query!(beatmap_metadata, beatmap_sets, beatmaps,);
