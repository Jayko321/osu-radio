diesel::table! {
    beatmap_sets (id) {
        id -> Integer,
        online_id -> Nullable<Integer>,
        hash -> Nullable<Text>,
        installation_id -> Nullable<Integer>,
    }
}
