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
