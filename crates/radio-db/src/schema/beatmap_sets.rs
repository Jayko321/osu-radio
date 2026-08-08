diesel::table! {
    beatmap_sets (id) {
        id -> Integer,
        online_id -> Nullable<Integer>,
        hash -> Nullable<Text>,
    }
}
