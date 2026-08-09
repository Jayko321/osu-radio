CREATE TABLE IF NOT EXISTS user_data (
    id INTEGER PRIMARY KEY
);

INSERT INTO user_data (id)
SELECT 1
WHERE NOT EXISTS (SELECT 1 FROM user_data WHERE id = 1);

CREATE TABLE IF NOT EXISTS osu_installations (
    id INTEGER PRIMARY KEY,
    user_data_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    root_path TEXT NOT NULL,
    marker_path TEXT NOT NULL,
    label TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_scanned_at TEXT,
    UNIQUE (marker_path),
    FOREIGN KEY (user_data_id) REFERENCES user_data(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS beatmap_sets (
    id INTEGER PRIMARY KEY,
    online_id INTEGER,
    hash TEXT,
    installation_id INTEGER,
    FOREIGN KEY (installation_id) REFERENCES osu_installations(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS audio_sources (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    location TEXT NOT NULL,
    UNIQUE (kind, location)
);

CREATE TABLE IF NOT EXISTS beatmap_metadata (
    id INTEGER PRIMARY KEY,
    title TEXT,
    title_unicode TEXT,
    artist TEXT,
    artist_unicode TEXT,
    author_online_id INTEGER,
    author_username TEXT,
    author_country_code TEXT,
    source TEXT,
    tags TEXT,
    user_tags TEXT,
    preview_time INTEGER,
    audio_source_id INTEGER,
    background_file TEXT,
    FOREIGN KEY (audio_source_id) REFERENCES audio_sources(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS beatmaps (
    id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    difficulty_name TEXT,
    bpm DOUBLE PRECISION,
    hash TEXT,
    beatmap_set_id INTEGER,
    metadata_id INTEGER,
    FOREIGN KEY (beatmap_set_id) REFERENCES beatmap_sets(id) ON DELETE CASCADE,
    FOREIGN KEY (metadata_id) REFERENCES beatmap_metadata(id) ON DELETE SET NULL
);
