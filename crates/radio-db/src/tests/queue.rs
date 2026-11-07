use super::*;
use crate::migrations::Migrator;
use crate::model::{PlaybackMode, QueueAvailability, QueueEntryId, QueueState, RepeatMode};
use sea_orm_migration::{MigratorTrait, SchemaManager};

pub(super) async fn migration_contracts(database: &Database, url: &str) {
    for (index, mode) in [
        (None, "stopped"),
        (Some(0), "playing"),
        (Some(1), "paused"),
        (Some(8), "stopped"),
    ] {
        populated_upgrade(database, url, index, mode).await;
    }
    waiting_migration(database, url).await;
    launch_source_migration(database, url).await;
    migration_rollback(database).await;
}

async fn fixture(database: &Database, index: Option<i64>, mode: &str) {
    Migrator::up(&database.connection, Some(11)).await.unwrap();
    sql(database, "INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (91,1,'lazer','\"/queue\"','\"/queue/client.realm\"')").await;
    sql(
        database,
        "INSERT INTO beatmap_sets (id,installation_id) VALUES (91,91)",
    )
    .await;
    sql(database, "INSERT INTO beatmap_metadata (hash,title,title_unicode,artist,artist_unicode) VALUES ('queue-meta','Library title','図書館','Artist','作者')").await;
    sql(database, "INSERT INTO audio_sources (id,kind,location) VALUES (91,'local','{\"Unix\":[255,47,97]}'), (92,'online','https://invalid/audio'), (93,'copied','./曲/audio.ogg')").await;
    sql(database, "INSERT INTO beatmaps (id,beatmap_set_id,hash,md5_hash,metadata_hash,audio_source_id,difficulty_name,background_path) VALUES (91,91,'native','md5','queue-meta',91,'Hard','/queue/cover')").await;
    sql(
        database,
        "INSERT INTO playlists (id,name) VALUES (91,'Keep playlist')",
    )
    .await;
    sql(database, "INSERT INTO playlist_items (id,playlist_id,source_kind,hash_kind,beatmap_hash,title,title_unicode,artist,artist_unicode,difficulty_name) VALUES (91,91,'lazer','md5','md5','Saved title','保存','Saved artist','演奏者','Insane')").await;
    sql(database, "INSERT INTO listening_history (audio_kind,source_location,last_played_at_ms,playback_token) VALUES ('local','kept-source',123456,8)").await;
    sql(database, "INSERT INTO audio_volume (audio_kind,source_location,volume_percent) VALUES ('local','kept-source',37)").await;
    sql(
        database,
        "UPDATE user_data SET global_volume_percent = 63, individual_volume_enabled = TRUE",
    )
    .await;
    if let Some(index) = index {
        sql(database, &format!("UPDATE playback_queue SET audio_source_ids = '[91,91,999,92,93,91,91,999]', playlist_item_ids = '[91,91,999,null,null,null,999,91]', current_index = {index}, mode = '{mode}', revision = 17, playback_token = 23")).await;
    }
}

#[allow(clippy::too_many_lines)] // One frozen v11 fixture exercises the full reversible migration and repository lifecycle.
async fn populated_upgrade(database: &Database, url: &str, index: Option<i64>, mode: &str) {
    fixture(database, index, mode).await;
    let library = read_v11_beatmap(database, 91).await;
    let other = Database::connect(url).await.unwrap();
    let (one, two) = tokio::join!(database.migrate(), other.migrate());
    one.unwrap();
    two.unwrap();
    let state = database.queue().get().await.unwrap();
    assert_eq!(database.beatmaps().get(91).await.unwrap(), library);
    assert_eq!(
        database
            .user_data()
            .audio_settings()
            .await
            .unwrap()
            .global_volume_percent,
        63
    );
    for (table, column, value) in [
        ("listening_history", "last_played_at_ms", 123_456_i64),
        ("audio_volume", "volume_percent", 37),
    ] {
        let row = database
            .connection
            .query_one_raw(sea_orm::Statement::from_string(
                database.connection.get_database_backend(),
                format!("SELECT CAST({column} AS BIGINT) AS value FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get::<i64>("", "value").unwrap(), value);
    }
    if let Some(index) = index {
        assert_eq!(state.audio_source_ids(), [91, 91, 999, 92, 93, 91, 91, 999]);
        assert_eq!(
            state.playlist_item_ids(),
            [
                Some(91),
                Some(91),
                Some(999),
                None,
                None,
                None,
                Some(999),
                Some(91)
            ]
        );
        assert_eq!(
            state.entries.iter().map(|e| e.id.get()).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(state.history_len, usize::try_from(index).unwrap());
        assert_eq!(
            state.current_entry_id.map(QueueEntryId::get),
            (index < 8).then_some(index.checked_add(1).unwrap())
        );
        assert_eq!(
            state.mode,
            match mode {
                "playing" => PlaybackMode::Playing,
                "paused" => PlaybackMode::Paused,
                _ => PlaybackMode::Stopped,
            }
        );
        assert_eq!(state.waiting_for_check, index < 8 && mode != "stopped");
        assert_eq!(state.revision, 17);
        assert_eq!(state.playback_token, 23);
        assert!(!state.shuffle && !state.repeat_launch);
        assert_eq!(state.repeat, RepeatMode::Off);
        assert!(state.entries.iter().all(|e| e.shuffle_order.is_none()));
        let first = &state.entries[0];
        assert_eq!(first.title.as_deref(), Some("Saved title"));
        assert_eq!(first.title_unicode.as_deref(), Some("保存"));
        assert_eq!(first.artist_unicode.as_deref(), Some("演奏者"));
        assert_eq!(first.difficulty_name.as_deref(), Some("Insane"));
        assert_eq!(
            first.audio_source.as_ref().unwrap().location(),
            "{\"Unix\":[255,47,97]}"
        );
        assert_eq!(first.playlist_source.as_ref().unwrap().hash_kind, "md5");
        assert_eq!(first.availability, QueueAvailability::Checking);
        assert_eq!(state.entries[2].audio_source, None);
        assert_eq!(state.entries[2].playlist_source, None);
        assert_eq!(state.entries[2].migration_audio_source_id, Some(999));
        assert_eq!(state.entries[2].migration_playlist_item_id, Some(999));
        assert_eq!(state.entries[2].title.as_deref(), Some("Unavailable track"));
        assert_eq!(
            state.entries[3].availability,
            QueueAvailability::Unavailable
        );
        assert_eq!(state.entries[4].availability, QueueAvailability::Checking);
        assert_eq!(state.entries[5].title.as_deref(), Some("Library title"));
        assert_eq!(state.entries[5].title_unicode.as_deref(), Some("図書館"));
        assert_eq!(state.entries[6].migration_playlist_item_id, Some(999));
        assert_eq!(state.entries[6].title.as_deref(), Some("Library title"));
        assert_eq!(state.entries[7].title_unicode.as_deref(), Some("保存"));
        assert_eq!(
            state.entries[7].availability,
            QueueAvailability::Unavailable
        );
    } else {
        assert_eq!(state, QueueState::default());
    }
    let reopened = Database::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(reopened.queue().get().await.unwrap(), state);
    // The frozen reconstruction must retain every field through populated down/up.
    downgrade_to(database, 11).await.unwrap();
    assert!(
        !SchemaManager::new(&database.connection)
            .has_table("queue_entries")
            .await
            .unwrap()
    );
    database.migrate().await.unwrap();
    assert_eq!(database.queue().get().await.unwrap(), state);
    if !state.entries.is_empty() {
        repository_contracts(database, &other, &state).await;
    }
    database.reset().await.unwrap();
    assert_eq!(database.queue().get().await.unwrap(), QueueState::default());
    drop_application_tables(database).await;
}

async fn save(database: &Database, state: &QueueState) -> anyhow::Result<()> {
    let transaction = database.begin().await?;
    transaction.user_data().lock().await?;
    transaction.queue().save(state).await?;
    transaction.commit().await
}

#[allow(clippy::too_many_lines)] // Validation, snapshot and downgrade guards share one persisted starting state.
async fn repository_contracts(database: &Database, other: &Database, state: &QueueState) {
    assert!(QueueEntryId::new(0).is_err());
    assert!(QueueEntryId::new(-1).is_err());
    assert_eq!(
        QueueEntryId::new(i64::MAX).unwrap().to_string(),
        i64::MAX.to_string()
    );
    assert!(
        database.queue().save(state).await.is_err(),
        "pool writes are forbidden"
    );
    let mut invalids = vec![];
    let mut bad = state.clone();
    bad.entries[1].id = bad.entries[0].id;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.entries[0].position = 1;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.current_entry_id = Some(QueueEntryId::new(999).unwrap());
    invalids.push(bad);
    let mut bad = state.clone();
    bad.history_len = 9;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.history_len = usize::MAX;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.entries[0].position = usize::MAX;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.current_entry_id = None;
    bad.mode = PlaybackMode::Playing;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.current_entry_id = None;
    bad.mode = PlaybackMode::Paused;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.revision = u64::MAX;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.playback_token = u64::MAX;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.entries[0].check_generation = u64::MAX;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.last_entry_id = 0;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.entries[0].playlist_source.as_mut().unwrap().hash_kind = "unknown".into();
    invalids.push(bad);
    let mut bad = state.clone();
    bad.entries.clear();
    bad.current_entry_id = None;
    bad.mode = PlaybackMode::Stopped;
    bad.history_len = 1;
    invalids.push(bad);
    // Keys exist exactly while shuffle is on.
    let mut bad = state.clone();
    bad.entries[0].shuffle_order = Some(1);
    invalids.push(bad);
    let mut bad = state.clone();
    bad.shuffle = true;
    invalids.push(bad);
    let mut bad = state.clone();
    bad.shuffle = true;
    for (key, entry) in (0..).zip(&mut bad.entries) {
        entry.shuffle_order = Some(key);
    }
    bad.entries[7].shuffle_order = None;
    invalids.push(bad);
    for bad in invalids {
        assert!(save(database, &bad).await.is_err(), "accepted {bad:?}");
        assert_eq!(database.queue().get().await.unwrap(), *state);
    }
    downgrade_guards(database, state).await;
    queue_modes_contracts(database, other, state).await;
    save_failure_rolls_back(database, state).await;
    let mut clear = state.clone();
    clear.entries.clear();
    clear.current_entry_id = None;
    clear.history_len = 0;
    clear.mode = PlaybackMode::Stopped;
    clear.waiting_for_check = false;
    clear.revision = clear.revision.checked_add(1).unwrap();
    // Explicit and drop rollback both restore list and singleton.
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    transaction.queue().save(&clear).await.unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(other.queue().get().await.unwrap(), *state);
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    transaction.queue().save(&clear).await.unwrap();
    drop(transaction);
    assert_eq!(other.queue().get().await.unwrap(), *state);
    // Freeze the read snapshot before an independent writer commits both projections.
    let read = database.begin_read().await.unwrap();
    assert_eq!(read.queue().get().await.unwrap(), *state);
    save(other, &clear).await.unwrap();
    assert_eq!(read.queue().get().await.unwrap(), *state);
    read.commit().await.unwrap();
    assert_eq!(database.queue().get().await.unwrap(), clear);
    let mut reused = clear.clone();
    reused.entries = state.entries.clone();
    reused.current_entry_id = state.current_entry_id;
    reused.history_len = state.history_len;
    reused.mode = state.mode;
    assert!(
        save(database, &reused).await.is_err(),
        "deleted IDs cannot be reissued"
    );
    let mut next = clear.clone();
    let mut entry = state.entries[0].clone();
    entry.id = next.allocate_id().unwrap();
    entry.position = 0;
    next.entries.push(entry);
    save(database, &next).await.unwrap();
    assert!(next.entries[0].id.get() > state.last_entry_id);
    // Each independently unrepresentable field must block downgrade before schema/history changes.
    let mut mutations = vec![next.clone()];
    let mut bad = next.clone();
    bad.last_entry_id = i64::MAX;
    mutations.push(bad);
    for candidate in mutations {
        save(database, &candidate).await.unwrap();
        assert!(downgrade_to(database, 11).await.is_err());
        assert_eq!(database.queue().get().await.unwrap(), candidate);
        assert_eq!(
            Migrator::get_applied_migrations(&database.connection)
                .await
                .unwrap()
                .len(),
            17
        );
        assert!(
            !SchemaManager::new(&database.connection)
                .has_table("playback_queue_v11")
                .await
                .unwrap()
        );
    }
    let mut maximum = database.queue().get().await.unwrap();
    assert!(maximum.allocate_id().is_err());
    maximum.revision = i64::MAX.try_into().unwrap();
    maximum.playback_token = i64::MAX.try_into().unwrap();
    maximum.entries[0].check_generation = i64::MAX.try_into().unwrap();
    save(database, &maximum).await.unwrap();
    assert_eq!(database.queue().get().await.unwrap(), maximum);
    sql(database, "DELETE FROM playlists WHERE id = 91").await;
    sql(database, "DELETE FROM osu_installations WHERE id = 91").await;
    sql(database, "DELETE FROM audio_sources WHERE id = 91").await;
    assert_eq!(
        database.queue().get().await.unwrap(),
        maximum,
        "advisory references never cascade queue snapshots"
    );
}

// Modes and order keys round-trip, the database rejects bad modes, and v15 cannot hold active modes.
async fn queue_modes_contracts(database: &Database, other: &Database, state: &QueueState) {
    let mut modes = state.clone();
    modes.shuffle = true;
    modes.repeat = RepeatMode::One;
    modes.repeat_launch = true;
    let count = i64::try_from(modes.entries.len()).unwrap();
    for (key, entry) in (0..count).rev().zip(&mut modes.entries) {
        entry.shuffle_order = Some(key);
    }
    save(database, &modes).await.unwrap();
    assert_eq!(other.queue().get().await.unwrap(), modes);
    assert!(
        database
            .connection
            .execute_unprepared("UPDATE playback_queue SET repeat_mode = 'sometimes'")
            .await
            .is_err()
    );
    assert!(downgrade_to(database, 11).await.is_err());
    assert_eq!(database.queue().get().await.unwrap(), modes);
    modes.shuffle = false;
    modes.repeat = RepeatMode::All;
    modes.repeat_launch = false;
    modes
        .entries
        .iter_mut()
        .for_each(|e| e.shuffle_order = None);
    save(database, &modes).await.unwrap();
    assert_eq!(other.queue().get().await.unwrap(), modes);
    assert!(downgrade_to(database, 11).await.is_err());
    assert_eq!(database.queue().get().await.unwrap(), modes);
    assert_eq!(
        Migrator::get_applied_migrations(&database.connection)
            .await
            .unwrap()
            .len(),
        17
    );
    save(database, state).await.unwrap();
}

async fn migration_rollback(database: &Database) {
    fixture(database, Some(1), "paused").await;
    sql(
        database,
        "UPDATE playback_queue SET playlist_item_ids = '[]'",
    )
    .await;
    assert!(database.migrate().await.is_err());
    assert_eq!(
        Migrator::get_applied_migrations(&database.connection)
            .await
            .unwrap()
            .len(),
        11
    );
    assert!(
        !SchemaManager::new(&database.connection)
            .has_table("queue_entries")
            .await
            .unwrap()
    );
    sql(
        database,
        "UPDATE playback_queue SET playlist_item_ids = '[91,91,999,null,null,null,999,91]'",
    )
    .await;
    // Fail after the entry table and populated rows have been created.
    sql(database, "CREATE TABLE playback_queue_v12 (probe INTEGER)").await;
    assert!(database.migrate().await.is_err());
    assert!(
        !SchemaManager::new(&database.connection)
            .has_table("queue_entries")
            .await
            .unwrap()
    );
    assert!(
        SchemaManager::new(&database.connection)
            .has_column("playback_queue", "audio_source_ids")
            .await
            .unwrap()
    );
    assert_eq!(
        Migrator::get_applied_migrations(&database.connection)
            .await
            .unwrap()
            .len(),
        11
    );
    sql(database, "DROP TABLE playback_queue_v12").await;
    database.migrate().await.unwrap();
    assert_eq!(database.queue().get().await.unwrap().entries.len(), 8);
    drop_application_tables(database).await;
}

async fn downgrade_guards(database: &Database, state: &QueueState) {
    let mut candidates = vec![];
    let mut bad = state.clone();
    bad.entries[0].title = Some("Changed snapshot".into());
    candidates.push(bad);
    let mut bad = state.clone();
    bad.entries[0].title_unicode = Some("Changed Unicode".into());
    candidates.push(bad);
    let mut bad = state.clone();
    bad.entries[0].artist = Some("Changed artist".into());
    candidates.push(bad);
    let mut bad = state.clone();
    bad.entries[0].artist_unicode = Some("Changed artist Unicode".into());
    candidates.push(bad);
    let mut bad = state.clone();
    bad.entries[0].difficulty_name = Some("Changed difficulty".into());
    candidates.push(bad);
    let mut bad = state.clone();
    bad.entries[0].availability = QueueAvailability::Available;
    bad.waiting_for_check = false;
    candidates.push(bad);
    let mut bad = state.clone();
    bad.entries[0].check_generation = 1;
    candidates.push(bad);
    let mut bad = state.clone();
    bad.entries[0].audio_source = Some(SourceType::Local("changed path".into()));
    candidates.push(bad);
    for candidate in candidates {
        save(database, &candidate).await.unwrap();
        assert!(downgrade_to(database, 11).await.is_err());
        assert_eq!(database.queue().get().await.unwrap(), candidate);
        assert_eq!(
            database
                .beatmaps()
                .get(91)
                .await
                .unwrap()
                .unwrap()
                .audio_source_id,
            Some(91)
        );
        assert_eq!(
            Migrator::get_applied_migrations(&database.connection)
                .await
                .unwrap()
                .len(),
            17
        );
        save(database, state).await.unwrap();
    }
    let mut boundary = state.clone();
    boundary.current_entry_id = None;
    boundary.mode = PlaybackMode::Stopped;
    boundary.waiting_for_check = false;
    boundary.history_len = 0;
    save(database, &boundary).await.unwrap();
    assert!(downgrade_to(database, 11).await.is_err());
    assert_eq!(database.queue().get().await.unwrap(), boundary);
    save(database, state).await.unwrap();
}
async fn save_failure_rolls_back(database: &Database, state: &QueueState) {
    #[cfg(feature = "sqlite")]
    sql(database, "CREATE TRIGGER reject_queue_entry BEFORE INSERT ON queue_entries WHEN NEW.position = 1 BEGIN SELECT RAISE(ABORT, 'queue save failure'); END").await;
    #[cfg(feature = "postgres")]
    {
        sql(database, "CREATE FUNCTION reject_queue_entry_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.position = 1 THEN RAISE EXCEPTION 'queue save failure'; END IF; RETURN NEW; END $$").await;
        sql(database, "CREATE TRIGGER reject_queue_entry BEFORE INSERT ON queue_entries FOR EACH ROW EXECUTE FUNCTION reject_queue_entry_fn()").await;
    }
    let mut changed = state.clone();
    changed.revision = changed.revision.checked_add(1).unwrap();
    assert!(save(database, &changed).await.is_err());
    assert_eq!(database.queue().get().await.unwrap(), *state);
    #[cfg(feature = "sqlite")]
    sql(database, "DROP TRIGGER reject_queue_entry").await;
    #[cfg(feature = "postgres")]
    {
        sql(database, "DROP TRIGGER reject_queue_entry ON queue_entries").await;
        sql(database, "DROP FUNCTION reject_queue_entry_fn()").await;
    }
}

// Keep all newer schema/history intact when an older downgrade guard rejects the operation.
async fn downgrade_to(database: &Database, version: usize) -> Result<(), sea_orm::DbErr> {
    let transaction = database.begin().await.unwrap();
    let count = Migrator::get_applied_migrations(&transaction.connection)
        .await
        .unwrap()
        .len();
    let result = Migrator::down(
        &transaction.connection,
        Some(u32::try_from(count.checked_sub(version).unwrap()).unwrap()),
    )
    .await;
    if result.is_ok() {
        transaction.commit().await.unwrap();
    } else {
        transaction.rollback().await.unwrap();
    }
    result
}

async fn waiting_migration(database: &Database, url: &str) {
    fixture(database, Some(0), "playing").await;
    Migrator::up(&database.connection, Some(1)).await.unwrap();
    assert!(
        !SchemaManager::new(&database.connection)
            .has_column("playback_queue", "waiting_for_check")
            .await
            .unwrap()
    );
    database.migrate().await.unwrap();
    let state = database.queue().get().await.unwrap();
    assert!(state.waiting_for_check);
    assert_eq!(state.entries.len(), 8);
    assert_eq!(state.mode, PlaybackMode::Playing);
    assert_eq!(
        (state.revision, state.playback_token, state.last_entry_id),
        (17, 23, 8)
    );
    downgrade_to(database, 12).await.unwrap();
    database.migrate().await.unwrap();
    assert_eq!(database.queue().get().await.unwrap(), state);
    let reopened = Database::connect(url).await.unwrap();
    assert_eq!(reopened.queue().get().await.unwrap(), state);
    for mutation in [0, 1, 2] {
        let mut bad = state.clone();
        match mutation {
            0 => bad.mode = PlaybackMode::Stopped,
            1 => {
                bad.current_entry_id = None;
                bad.mode = PlaybackMode::Stopped;
            }
            _ => bad.entries[0].availability = QueueAvailability::Available,
        }
        assert!(save(database, &bad).await.is_err());
        assert_eq!(database.queue().get().await.unwrap(), state);
    }
    // A running assignment can be rechecking without waiting: v12 cannot express that intent.
    let mut assigned = state.clone();
    assigned.waiting_for_check = false;
    assigned.launch_source = Some(crate::model::QueueLaunchSource {
        audio_source_id: assigned.entries[0].audio_source_id.unwrap(),
        source: assigned.entries[0].audio_source.clone().unwrap(),
        check_generation: assigned.entries[0].check_generation,
    });
    save(database, &assigned).await.unwrap();
    let error = downgrade_to(database, 12).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("waiting intent cannot be reconstructed")
    );
    assert_eq!(database.queue().get().await.unwrap(), assigned);
    assert_eq!(
        Migrator::get_applied_migrations(&database.connection)
            .await
            .unwrap()
            .len(),
        17
    );
    assigned.entries[0].availability = QueueAvailability::Available;
    save(database, &assigned).await.unwrap();
    downgrade_to(database, 12).await.unwrap();
    database.migrate().await.unwrap();
    assert_eq!(database.queue().get().await.unwrap(), assigned);
    drop_application_tables(database).await;
}

async fn launch_source_migration(database: &Database, url: &str) {
    for (mode, waiting, complete) in [
        ("playing", false, true),
        ("paused", false, true),
        ("playing", true, true),
        ("stopped", false, true),
        ("paused", false, false),
    ] {
        fixture(database, Some(0), mode).await;
        Migrator::up(&database.connection, Some(2)).await.unwrap();
        sql(
            database,
            &format!("UPDATE playback_queue SET waiting_for_check = {waiting}"),
        )
        .await;
        if !complete {
            sql(
                database,
                "UPDATE queue_entries SET audio_kind = NULL, audio_location = NULL WHERE id = 1",
            )
            .await;
        }
        database.migrate().await.unwrap();
        let state = database.queue().get().await.unwrap();
        assert_eq!(
            state.launch_source.is_some(),
            mode != "stopped" && !waiting && complete
        );
        if let Some(source) = &state.launch_source {
            assert_eq!(source.audio_source_id, 91);
            assert_eq!(
                source.source,
                state.entries[0].audio_source.clone().unwrap()
            );
            assert_eq!(source.check_generation, 0);
        }
        let reopened = Database::connect(url).await.unwrap();
        assert_eq!(reopened.queue().get().await.unwrap(), state);
        downgrade_to(database, 13).await.unwrap();
        database.migrate().await.unwrap();
        assert_eq!(database.queue().get().await.unwrap(), state);
        if state.launch_source.is_some() {
            let mut rebound = state.clone();
            rebound.entries[0].audio_source = Some(SourceType::Copied("new/source".into()));
            rebound.entries[0].check_generation = 1;
            save(database, &rebound).await.unwrap();
            let error = downgrade_to(database, 13).await.unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("launch source cannot be reconstructed")
            );
            assert_eq!(database.queue().get().await.unwrap(), rebound);
            assert_eq!(
                Migrator::get_applied_migrations(&database.connection)
                    .await
                    .unwrap()
                    .len(),
                17
            );
        }
        for invalid in [
            "UPDATE playback_queue SET launch_audio_kind = NULL, launch_audio_source_id = 91",
            "UPDATE playback_queue SET launch_audio_source_id = 91, launch_audio_kind = 'local', launch_audio_location = '/x', launch_check_generation = -1",
        ] {
            assert!(
                database
                    .connection
                    .execute_unprepared(invalid)
                    .await
                    .is_err()
            );
        }
        database.reset().await.unwrap();
        assert_eq!(database.queue().get().await.unwrap(), QueueState::default());
        drop_application_tables(database).await;
    }
}
