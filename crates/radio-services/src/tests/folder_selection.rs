use super::*;

/// One Stable set with any number of difficulties, using the current database layout.
fn stable_database(difficulties: &[&str]) -> Vec<u8> {
    fn text(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend([0x0b, u8::try_from(value.len()).unwrap()]);
        bytes.extend(value.as_bytes());
    }
    let count = i32::try_from(difficulties.len()).unwrap();
    let mut bytes = 20_250_108_i32.to_le_bytes().to_vec();
    bytes.extend(1_i32.to_le_bytes());
    bytes.push(1);
    bytes.extend([0; 8]);
    bytes.push(0);
    bytes.extend(count.to_le_bytes());
    for difficulty in difficulties {
        for value in [
            "Artist",
            "Artist",
            "Song",
            "Song",
            "Mapper",
            difficulty,
            "audio.mp3",
            "hash",
            "map.osu",
        ] {
            text(&mut bytes, value);
        }
        bytes.push(4); // Ranked status.
        bytes.extend([0; 6 + 8 + 16 + 8 + 16 + 4 + 4 + 4 + 4 + 4]);
        bytes.extend(42_i32.to_le_bytes()); // Shared set ID.
        bytes.extend([0; 4 + 4 + 2 + 4 + 1]);
        bytes.extend([0; 2]); // Source and tags.
        bytes.extend([0; 2]); // Online offset.
        bytes.push(0); // Font.
        bytes.push(1); // Unplayed.
        bytes.extend([0; 8 + 1]);
        text(&mut bytes, "42 Song");
        bytes.extend([0; 8 + 5 + 4 + 1]);
    }
    bytes.extend(1_i32.to_le_bytes());
    bytes
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn atomic_registration_rollback_duplicates_and_shared_removal() {
    let directory = tempfile::tempdir().unwrap();
    let database =
        TestDatabase::connect(directory.path().join("selection.sqlite").to_str().unwrap()).await;
    let installations = database.osu_installations();
    let source = snapshot("Shared song", "/shared/song.mp3");
    let first = installations
        .register_snapshot(&marker("first"), &source)
        .await
        .unwrap();
    assert!(first.was_created());
    assert!(first.installation().last_scanned_at.is_some());
    // Fail after registration and set insertion, not only during source validation.
    sql(&database, "CREATE TRIGGER fail_selection BEFORE INSERT ON beatmaps BEGIN SELECT RAISE(ABORT, 'forced import failure'); END").await;
    assert!(
        installations
            .register_snapshot(&marker("failed"), &source)
            .await
            .is_err()
    );
    assert_eq!(installations.all().await.unwrap().len(), 1);
    assert_counts(&database, [1, 2, 1, 1]).await;
    sql(&database, "DROP TRIGGER fail_selection").await;
    let second = installations
        .register_snapshot(&marker("second"), &source)
        .await
        .unwrap();
    let duplicate = installations
        .register_snapshot(&marker("first"), &[])
        .await
        .unwrap();
    assert!(!duplicate.was_created());
    assert_eq!(duplicate.installation(), first.installation());
    assert_counts(&database, [2, 4, 1, 1]).await;
    assert!(installations.delete(first.installation().id).await.unwrap());
    assert_counts(&database, [1, 2, 1, 1]).await;
    assert!(
        installations
            .delete(second.installation().id)
            .await
            .unwrap()
    );
    assert_counts(&database, [0, 0, 0, 0]).await;
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn previews_count_zero_and_never_register_and_bad_reads_leave_no_folder() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("osu!.db");
    let mut bytes = 20_250_108_i32.to_le_bytes().to_vec();
    bytes.extend(0_i32.to_le_bytes());
    bytes.extend([1; 1]);
    bytes.extend([0; 8]);
    bytes.push(0);
    bytes.extend(0_i32.to_le_bytes());
    bytes.extend(1_i32.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();
    let services = Services::connect(":memory:").await.unwrap();
    services.migrate().await.unwrap();
    let service = services.osu_installations();
    assert_eq!(service.folder_metadata(path.clone()).await.unwrap(), 0);
    assert!(service.all().await.unwrap().is_empty());
    let source = stable_database(&["Easy", "Hard"]);
    std::fs::create_dir_all(directory.path().join("Songs/42 Song")).unwrap();
    std::fs::write(
        directory.path().join("Songs/42 Song/map.osu"),
        b"[Events]\n",
    )
    .unwrap();
    std::fs::write(&path, &source).unwrap();
    assert_eq!(service.folder_metadata(path.clone()).await.unwrap(), 2);
    assert!(service.all().await.unwrap().is_empty());
    let mut discovery = service
        .discover_folders(crate::DiscoveryOptions {
            roots: vec![directory.path().to_path_buf()],
            ..Default::default()
        })
        .await
        .unwrap();
    let candidate = discovery.next().await.unwrap();
    assert_eq!(
        candidate.marker.marker_path,
        std::fs::canonicalize(&path).unwrap()
    );
    assert_eq!(candidate.registered_id, None);
    assert!(discovery.next().await.is_none());
    std::fs::write(&path, b"broken db").unwrap();
    assert!(service.import_folder(path.clone()).await.is_err());
    assert!(service.all().await.unwrap().is_empty());
    std::fs::write(&path, &source).unwrap();
    let stored = service.import_folder(path.clone()).await.unwrap();
    let sets = services
        .beatmap_sets()
        .all_with_audio_sources()
        .await
        .unwrap();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].beatmaps.len(), 2);
    std::fs::write(&path, b"broken db").unwrap();
    assert_eq!(
        service.import_folder(path).await.unwrap().installation(),
        stored.installation()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn discovery_and_import_resolve_aliases_to_registered_ids() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    let alias = directory.path().join("alias");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("osu!.db"), stable_database(&[])).unwrap();
    symlink(&source, &alias).unwrap();
    let services = Services::connect(":memory:").await.unwrap();
    services.migrate().await.unwrap();
    let service = services.osu_installations();
    let folder = service
        .register_folder(alias.clone(), Some("Stored".into()))
        .await
        .unwrap();
    let mut discovery = service
        .discover_folders(crate::DiscoveryOptions {
            roots: vec![alias, source.clone()],
            ..Default::default()
        })
        .await
        .unwrap();
    let candidate = discovery.next().await.unwrap();
    assert_eq!(candidate.registered_id, Some(folder.installation().id));
    assert_eq!(
        candidate.marker.marker_path,
        std::fs::canonicalize(source.join("osu!.db")).unwrap()
    );
    assert!(discovery.next().await.is_none());
    let imported = service.import_folder(source.join("osu!.db")).await.unwrap();
    assert!(!imported.was_created());
    assert_eq!(imported.installation(), folder.installation());
}
