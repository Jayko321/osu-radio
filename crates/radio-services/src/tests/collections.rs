use super::*;
use radio_core::import_types::{ImportedCollection, ImportedSnapshot};

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccc";
const MISSING: &str = "dddddddddddddddddddddddddddddddd";

fn collection(id: &str, name: &str, hashes: &[&str]) -> ImportedCollection {
    ImportedCollection {
        source_id: id.into(),
        name: name.into(),
        beatmap_md5_hashes: hashes.iter().map(|hash| (*hash).into()).collect(),
    }
}

fn imported(title: &str) -> ImportedSnapshot {
    let mut sets = snapshot(title, "/collections/shared.mp3");
    sets[0].beatmaps[0].md5_hash = Some(A.into());
    sets[0].beatmaps[1].md5_hash = Some(B.into());
    sets[0].beatmaps.push(ImportedBeatmap {
        md5_hash: Some(C.into()),
        hash: Some(A.into()),
        difficulty_name: Some("Extra".into()),
        bpm: None,
        metadata: Some(metadata(title)),
    });
    ImportedSnapshot {
        beatmap_sets: sets,
        collections: vec![
            collection("guid-one", "好き", &[A, B, A, MISSING]),
            collection("guid-empty", "", &[]),
        ],
    }
}

#[allow(clippy::too_many_lines)] // One shared SQLite/PostgreSQL workflow covers full collection replacement and retention.
pub(super) async fn contracts(database: &TestDatabase) {
    let original_playlist_ids: std::collections::HashSet<_> = database
        .playlists()
        .all()
        .await
        .unwrap()
        .into_iter()
        .map(|playlist| playlist.id)
        .collect();
    let marker = marker("collections");
    let service = database.osu_installations();
    let initial = imported("First title");
    let registered = service
        .register_imported_snapshot(&marker, &initial)
        .await
        .unwrap();
    let folder_id = registered.installation().id;
    let playlists = database.playlists().all().await.unwrap();
    let id = playlists.iter().find(|row| row.name == "好き").unwrap().id;
    let empty_id = playlists
        .iter()
        .find(|row| row.name == "Unnamed collection")
        .unwrap()
        .id;
    assert_eq!(
        database
            .playlists()
            .get(empty_id)
            .await
            .unwrap()
            .unwrap()
            .items
            .len(),
        0
    );
    let original = database.playlists().get(id).await.unwrap().unwrap();
    assert_eq!(original.items.len(), 3, "duplicate MD5 is one difficulty");
    assert_eq!(
        original.items[0].audio_source_id,
        original.items[1].audio_source_id
    );
    assert_ne!(original.items[0].beatmap_id, original.items[1].beatmap_id);
    assert!(original.items[2].audio_source_id.is_none());
    assert_eq!(original.items[0].title.as_deref(), Some("First title"));
    assert_ne!(
        original.items[0].beatmap_hash, "beatmap-Easy",
        "lazer native Hash is not MD5"
    );
    assert!(
        playlists
            .iter()
            .find(|row| row.id == id)
            .unwrap()
            .cover_beatmap_id
            .is_some()
    );
    let png = super::playlists::cover_png(512, 512);
    database.playlists().set_cover(id, &png).await.unwrap();
    database
        .playlists()
        .rename(id, "Manual name")
        .await
        .unwrap();
    database
        .playlists()
        .add_items(id, &[original.items[0].beatmap_id.unwrap()])
        .await
        .unwrap();
    assert_eq!(
        database
            .playlists()
            .get(id)
            .await
            .unwrap()
            .unwrap()
            .items
            .len(),
        3,
        "manual add of imported difficulty is idempotent"
    );
    let set = database
        .beatmap_sets()
        .for_installation(folder_id)
        .await
        .unwrap()
        .remove(0);
    let extra = database.beatmaps().for_set(set.id).await.unwrap().remove(2);
    let manual = database
        .playlists()
        .create("Native MD5 collision")
        .await
        .unwrap();
    let manual = database
        .playlists()
        .add_items(manual.id, &[extra.id])
        .await
        .unwrap();
    assert_eq!(manual.items[0].beatmap_hash, original.items[0].beatmap_hash);
    assert_ne!(
        manual.items[0].beatmap_id, original.items[0].beatmap_id,
        "hash kind separates equal native and MD5 strings"
    );
    assert_eq!(manual.items[0].difficulty_name.as_deref(), Some("Extra"));
    database
        .playlists()
        .add_items(id, &[extra.id])
        .await
        .unwrap();
    database
        .playlists()
        .remove_item(id, original.items[1].id)
        .await
        .unwrap();
    let mut changed = imported("Updated title");
    changed.collections = vec![collection("guid-one", "Renamed lazer", &[B, A, MISSING])];
    service
        .replace_imported_snapshot(folder_id, &changed)
        .await
        .unwrap();
    let updated = database.playlists().get(id).await.unwrap().unwrap();
    assert_eq!(updated.name, "Renamed lazer");
    assert_eq!(
        updated
            .items
            .iter()
            .map(|item| item.beatmap_hash.as_str())
            .collect::<Vec<_>>(),
        [A, MISSING, B],
        "remaining items retain their order, restored members append"
    );
    assert_eq!(updated.items[0].id, original.items[0].id);
    assert_eq!(updated.items[1].id, original.items[2].id);
    assert_eq!(updated.items[0].title.as_deref(), Some("Updated title"));
    assert_eq!(database.playlists().cover(id).await.unwrap(), Some(png));
    assert_eq!(
        database
            .playlists()
            .all()
            .await
            .unwrap()
            .iter()
            .find(|row| row.id == id)
            .unwrap()
            .custom_cover_revision,
        Some(1)
    );
    assert!(
        database.playlists().get(empty_id).await.unwrap().is_some(),
        "omitted collection stays saved"
    );
    service
        .replace_imported_snapshot(folder_id, &changed)
        .await
        .unwrap();
    let repeated = database.playlists().get(id).await.unwrap().unwrap();
    assert_eq!(
        repeated
            .items
            .iter()
            .map(|item| (item.id, &item.beatmap_hash))
            .collect::<Vec<_>>(),
        updated
            .items
            .iter()
            .map(|item| (item.id, &item.beatmap_hash))
            .collect::<Vec<_>>(),
        "repeat import preserves membership IDs and order"
    );
    let updated = repeated;
    let folder_before = service.get(folder_id).await.unwrap().unwrap();
    install_failure(database).await;
    let mut failure = changed.clone();
    failure.beatmap_sets[0].beatmaps[0].metadata = Some(metadata("Must roll back"));
    failure.collections[0] = collection("guid-one", "Must roll back", &[B]);
    failure
        .collections
        .push(collection("guid-failure", "Failure", &[C]));
    assert!(
        service
            .replace_imported_snapshot(folder_id, &failure)
            .await
            .is_err()
    );
    remove_failure(database).await;
    assert_eq!(
        service.get(folder_id).await.unwrap().unwrap(),
        folder_before
    );
    assert_eq!(
        database.playlists().get(id).await.unwrap().unwrap(),
        updated
    );
    assert_eq!(
        database
            .beatmap_sets()
            .for_installation(folder_id)
            .await
            .unwrap()[0]
            .id,
        updated.items[0].beatmap_set_id.unwrap(),
        "library replacement also rolls back"
    );
    let empty_snapshot = ImportedSnapshot {
        beatmap_sets: vec![],
        collections: changed.collections.clone(),
    };
    service
        .replace_imported_snapshot(folder_id, &empty_snapshot)
        .await
        .unwrap();
    let missing = database.playlists().get(id).await.unwrap().unwrap();
    assert!(
        missing
            .items
            .iter()
            .all(|item| item.audio_source_id.is_none())
    );
    assert_eq!(missing.items[0].title.as_deref(), Some("Updated title"));
    assert_eq!(missing.items[0].id, updated.items[0].id);
    service
        .replace_imported_snapshot(folder_id, &changed)
        .await
        .unwrap();
    assert!(
        database.playlists().get(id).await.unwrap().unwrap().items[0]
            .audio_source_id
            .is_some()
    );
    service.delete(folder_id).await.unwrap();
    assert!(
        database
            .playlists()
            .get(id)
            .await
            .unwrap()
            .unwrap()
            .items
            .iter()
            .all(|item| item.audio_source_id.is_none())
    );
    let new_folder = service
        .register_imported_snapshot(&marker, &changed)
        .await
        .unwrap();
    assert_ne!(new_folder.installation().id, folder_id);
    assert_eq!(
        database.playlists().get(id).await.unwrap().unwrap().items[0].id,
        updated.items[0].id
    );
    assert!(
        database.playlists().get(id).await.unwrap().unwrap().items[0]
            .audio_source_id
            .is_some()
    );
    // Stable identities include exact original names and occurrences; rename creates another playlist.
    let mut stable_marker = marker.clone();
    stable_marker.kind = OsuKind::Stable;
    stable_marker.marker_path = "/test/stable-collections/osu!.db".into();
    stable_marker.root_path = "/test/stable-collections".into();
    let stable = ImportedSnapshot {
        beatmap_sets: vec![],
        collections: vec![
            collection("[\"Same\",0]", "Same", &[]),
            collection("[\"Same\",1]", "Same", &[MISSING]),
        ],
    };
    let stable_folder = service
        .register_imported_snapshot(&stable_marker, &stable)
        .await
        .unwrap();
    let same_ids: Vec<_> = database
        .playlists()
        .all()
        .await
        .unwrap()
        .iter()
        .filter(|row| row.name == "Same")
        .map(|row| row.id)
        .collect();
    assert_eq!(same_ids.len(), 2);
    service
        .replace_imported_snapshot(
            stable_folder.installation().id,
            &ImportedSnapshot {
                beatmap_sets: vec![],
                collections: vec![collection("[\"Different\",0]", "Different", &[])],
            },
        )
        .await
        .unwrap();
    for id in same_ids {
        assert!(database.playlists().get(id).await.unwrap().is_some());
    }
    assert!(
        database
            .playlists()
            .all()
            .await
            .unwrap()
            .iter()
            .any(|row| row.name == "Different")
    );
    service.delete(new_folder.installation().id).await.unwrap();
    service
        .delete(stable_folder.installation().id)
        .await
        .unwrap();
    for playlist in database.playlists().all().await.unwrap() {
        if !original_playlist_ids.contains(&playlist.id) {
            database.playlists().delete(playlist.id).await.unwrap();
        }
    }
}

async fn install_failure(database: &TestDatabase) {
    #[cfg(feature = "sqlite")]
    sql(database, &format!("CREATE TRIGGER fail_collection BEFORE INSERT ON playlist_items WHEN NEW.beatmap_hash = '{C}' BEGIN SELECT RAISE(ABORT, 'forced collection failure'); END")).await;
    #[cfg(feature = "postgres")]
    {
        sql(database, &format!("CREATE FUNCTION fail_collection() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.beatmap_hash = '{C}' THEN RAISE EXCEPTION 'forced collection failure'; END IF; RETURN NEW; END $$")).await;
        sql(database, "CREATE TRIGGER fail_collection BEFORE INSERT ON playlist_items FOR EACH ROW EXECUTE FUNCTION fail_collection()").await;
    }
}
async fn remove_failure(database: &TestDatabase) {
    #[cfg(feature = "sqlite")]
    sql(database, "DROP TRIGGER fail_collection").await;
    #[cfg(feature = "postgres")]
    {
        sql(database, "DROP TRIGGER fail_collection ON playlist_items").await;
        sql(database, "DROP FUNCTION fail_collection()").await;
    }
}
