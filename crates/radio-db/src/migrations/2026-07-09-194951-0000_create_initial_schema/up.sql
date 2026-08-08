CREATE TABLE IF NOT EXISTS beatmap_sets (
    id INTEGER PRIMARY KEY,
    online_id INTEGER,
    hash TEXT
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
