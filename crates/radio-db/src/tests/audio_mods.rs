use super::*;
use crate::{
    migrations::Migrator,
    model::{AudioModsMode as Mode, AudioModsProfile as Profile, AudioModsSettings},
};
use sea_orm_migration::MigratorTrait;

#[allow(clippy::too_many_lines)] // Populated v14 upgrade, durable writes, snapshot, rollback and reset share one isolated database.
pub(super) async fn contracts(database: &Database, url: &str) {
    Migrator::up(&database.connection, Some(14)).await.unwrap();
    sql(database, "INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (91,1,'lazer','\"/mods\"','\"/mods/client.realm\"')").await;
    sql(
        database,
        "INSERT INTO beatmap_sets (id,installation_id) VALUES (91,91)",
    )
    .await;
    sql(
        database,
        "INSERT INTO beatmap_metadata (hash,title) VALUES ('mods-meta','Keep library')",
    )
    .await;
    sql(
        database,
        "INSERT INTO audio_sources (id,kind,location) VALUES (91,'local','/mods/audio')",
    )
    .await;
    sql(database, "INSERT INTO beatmaps (id,beatmap_set_id,hash,metadata_hash,audio_source_id) VALUES (91,91,'mods-map','mods-meta',91)").await;
    sql(
        database,
        "INSERT INTO playlists (id,name) VALUES (91,'Keep playlist')",
    )
    .await;
    sql(database, "INSERT INTO playlist_items (id,playlist_id,source_kind,beatmap_hash) VALUES (91,91,'lazer','mods-map')").await;
    sql(database, "INSERT INTO queue_entries (id,position,audio_kind,audio_location,audio_source_id,availability,check_generation) VALUES (1,0,'local','/mods/audio',91,'available',0)").await;
    sql(database, "UPDATE playback_queue SET current_entry_id=1,last_entry_id=1,mode='playing',revision=7,playback_token=8,launch_audio_source_id=91,launch_audio_kind='local',launch_audio_location='/mods/audio',launch_check_generation=0").await;
    sql(
        database,
        "UPDATE user_data SET individual_volume_enabled=TRUE,global_volume_percent=63",
    )
    .await;
    let source = SourceType::Local("/mods/audio".into());
    database.audio_volume().set(&source, 37).await.unwrap();
    database
        .listening_history()
        .record_started(&source, 8, 1234)
        .await
        .unwrap();
    let map = database.beatmaps().get(91).await.unwrap();
    let playlist = database.playlists().get(91).await.unwrap();
    let queue = database.queue().get().await.unwrap();
    let other = Database::connect(url).await.unwrap();
    let (one, two) = tokio::join!(database.migrate(), other.migrate());
    one.unwrap();
    two.unwrap();
    assert_eq!(database.beatmaps().get(91).await.unwrap(), map);
    assert_eq!(database.playlists().get(91).await.unwrap(), playlist);
    assert_eq!(database.queue().get().await.unwrap(), queue);
    assert_eq!(
        database.user_data().audio_settings().await.unwrap(),
        crate::model::AudioSettings {
            individual_volume_enabled: true,
            global_volume_percent: 63
        }
    );
    assert_eq!(
        database
            .audio_volume()
            .for_audio_sources(&[91])
            .await
            .unwrap()
            .get(&91),
        Some(&37)
    );
    assert_eq!(
        database
            .listening_history()
            .for_audio_sources(&[91])
            .await
            .unwrap()
            .get(&91),
        Some(&1234)
    );
    assert_eq!(
        database.user_data().audio_mods().await.unwrap(),
        AudioModsSettings::default()
    );
    assert!(
        database
            .audio_mods()
            .for_sources(std::slice::from_ref(&source))
            .await
            .unwrap()
            .is_empty()
    );
    let dt = Profile {
        mode: Mode::Dt,
        speed_percent: 120,
        adjust_pitch: true,
    };
    let ht = Profile {
        mode: Mode::Ht,
        speed_percent: 75,
        adjust_pitch: false,
    };
    let settings = AudioModsSettings {
        individual_mods_enabled: true,
        global_profile: ht,
    };
    let writer = database.begin().await.unwrap();
    writer.user_data().lock().await.unwrap();
    writer
        .user_data()
        .update_audio_mods(settings)
        .await
        .unwrap();
    writer.audio_mods().set(&source, dt).await.unwrap();
    writer.commit().await.unwrap();
    let reopened = Database::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(reopened.user_data().audio_mods().await.unwrap(), settings);
    assert_eq!(
        reopened
            .audio_mods()
            .for_audio_sources(&vec![91; 1002])
            .await
            .unwrap()
            .get(&91),
        Some(&dt)
    );
    assert_eq!(
        reopened
            .audio_mods()
            .for_sources(&vec![source.clone(); 502])
            .await
            .unwrap()
            .get(&(source.kind().into(), source.location().into())),
        Some(&dt)
    );
    let reader = database.begin_read().await.unwrap();
    assert_eq!(reader.user_data().audio_mods().await.unwrap(), settings);
    other.audio_mods().set(&source, ht).await.unwrap();
    assert_eq!(
        reader
            .audio_mods()
            .for_audio_sources(&[91])
            .await
            .unwrap()
            .get(&91),
        Some(&dt)
    );
    reader.commit().await.unwrap();
    let writer = database.begin().await.unwrap();
    writer.user_data().lock().await.unwrap();
    writer.audio_mods().set(&source, dt).await.unwrap();
    writer
        .user_data()
        .update_audio_mods(AudioModsSettings::default())
        .await
        .unwrap();
    writer.rollback().await.unwrap();
    assert_eq!(other.user_data().audio_mods().await.unwrap(), settings);
    assert_eq!(
        other
            .audio_mods()
            .for_audio_sources(&[91])
            .await
            .unwrap()
            .get(&91),
        Some(&ht)
    );
    constraints(database, &source).await;
    // A downgrade removes only mods and can upgrade again without losing existing models.
    Migrator::down(&database.connection, Some(1)).await.unwrap();
    database.migrate().await.unwrap();
    assert_eq!(database.beatmaps().get(91).await.unwrap(), map);
    assert_eq!(database.playlists().get(91).await.unwrap(), playlist);
    assert_eq!(database.queue().get().await.unwrap(), queue);
    database.audio_mods().set(&source, dt).await.unwrap();
    database.reset().await.unwrap();
    assert_eq!(
        database.user_data().audio_mods().await.unwrap(),
        AudioModsSettings::default()
    );
    assert!(
        database
            .audio_mods()
            .for_sources(&[source])
            .await
            .unwrap()
            .is_empty()
    );
    drop_application_tables(database).await;
}

async fn constraints(database: &Database, source: &SourceType) {
    for (mode, speed, pitch, valid) in [
        ("none", 100, false, true),
        ("none", 100, true, false),
        ("none", 99, false, false),
        ("dt", 100, false, false),
        ("dt", 101, false, true),
        ("dt", 200, true, true),
        ("dt", 201, true, false),
        ("ht", 49, false, false),
        ("ht", 50, true, true),
        ("ht", 99, false, true),
        ("ht", 100, false, false),
        ("nc", 150, true, false),
        ("invalid", 100, false, false),
    ] {
        let result = database.connection.execute_unprepared(&format!("UPDATE user_data SET global_mods_mode='{mode}',global_mods_speed_percent={speed},global_mods_adjust_pitch={pitch}")).await;
        assert_eq!(result.is_ok(), valid, "global {mode}/{speed}/{pitch}");
        let result = database
            .connection
            .execute_unprepared(&format!(
                "UPDATE audio_mods SET mode='{mode}',speed_percent={speed},adjust_pitch={pitch}"
            ))
            .await;
        assert_eq!(result.is_ok(), valid, "individual {mode}/{speed}/{pitch}");
        if let Ok(mode) = mode.parse() {
            let profile = Profile {
                mode,
                speed_percent: speed,
                adjust_pitch: pitch,
            };
            assert_eq!(profile.is_valid(), valid);
            assert_eq!(
                database.audio_mods().set(source, profile).await.is_ok(),
                valid
            );
        }
    }
    #[cfg(feature = "sqlite")]
    for statement in [
        "UPDATE user_data SET global_mods_mode='dt',global_mods_speed_percent=101.5",
        "UPDATE audio_mods SET mode='dt',speed_percent=101.5",
    ] {
        assert!(
            database
                .connection
                .execute_unprepared(statement)
                .await
                .is_err(),
            "{statement}"
        );
    }
    let invalid = Profile {
        mode: Mode::None,
        speed_percent: 150,
        adjust_pitch: false,
    };
    assert!(!invalid.is_valid());
    assert!(database.audio_mods().set(source, invalid).await.is_err());
    assert!(
        database
            .user_data()
            .update_audio_mods(AudioModsSettings {
                individual_mods_enabled: false,
                global_profile: invalid
            })
            .await
            .is_err()
    );
    assert!(
        database
            .connection
            .execute_unprepared("INSERT INTO audio_mods VALUES ('bad','/bad','none',100,FALSE)")
            .await
            .is_err()
    );
}
