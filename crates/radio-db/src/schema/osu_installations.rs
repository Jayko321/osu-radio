diesel::table! {
    osu_installations (id) {
        id -> Integer,
        user_data_id -> Integer,
        kind -> Text,
        root_path -> Text,
        marker_path -> Text,
        label -> Nullable<Text>,
        enabled -> Bool,
        last_scanned_at -> Nullable<Text>,
    }
}
