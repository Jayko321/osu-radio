use super::*;
use std::collections::HashSet;

fn paths(sets: &[crate::BeatmapSetWithAudio]) -> HashSet<String> {
    sets.iter()
        .flat_map(|set| &set.audio_sources)
        .map(|audio| audio.s_type.location().to_owned())
        .collect()
}

#[allow(clippy::too_many_lines)]
pub(super) async fn contracts(database: &TestDatabase, other: &TestDatabase) {
    let id = register(database, "search").await;
    let mut imported = snapshot("Starlight", "/search/shared.mp3");
    for map in &mut imported[0].beatmaps {
        let meta = map.metadata.as_mut().unwrap();
        meta.artist = Some("Orchestra".into());
        meta.artist_unicode = Some("ЁЖ".into());
        meta.title_unicode = Some("ЗВЁЗДЫ".into());
        meta.user_tags.clear();
        meta.tags = Some("ROCKET a+b%_&?#".into());
    }
    // A third difficulty in the same set references different audio.
    let mut separate = imported[0].beatmaps[0].clone();
    separate.difficulty_name = Some("Insane".into());
    let meta = separate.metadata.as_mut().unwrap();
    meta.audio_file = Some("other.mp3".into());
    meta.title = Some("Other".into());
    imported[0].files.push(RealmNamedFileUsage {
        filename: Some("other.mp3".into()),
        file: Some(RealmFile {
            hash: None,
            resolved_path: Some("/search/other.mp3".into()),
        }),
    });
    imported[0].beatmaps.push(separate);
    let mut another = snapshot("Alias", "/search/shared.mp3").remove(0);
    for map in &mut another.beatmaps {
        let meta = map.metadata.as_mut().unwrap();
        meta.user_tags.clear();
        meta.artist = Some("Guest".into());
    }
    imported.push(another);
    database
        .osu_installations()
        .replace_snapshot(id, &imported)
        .await
        .unwrap();
    let service = database.beatmap_sets();
    let all = service.all_with_audio_sources().await.unwrap();
    for query in ["", " \t\n\u{2003}"] {
        assert_eq!(service.search_with_audio_sources(query).await.unwrap(), all);
    }
    for query in [
        "roc", "ROCK", "orCH", "ёж", "звёз", "a+b%_&?#", "%", "_", "a+b", "&?#",
    ] {
        assert_eq!(
            paths(&service.search_with_audio_sources(query).await.unwrap()),
            HashSet::from(["/search/shared.mp3".into(), "/search/other.mp3".into()]),
            "{query}"
        );
    }
    for query in [
        "star",
        "hard",
        "roc HARD",
        " easy\tHARD\n",
        "guest starlight",
        "hard звёз ёж roc",
    ] {
        let found = service.search_with_audio_sources(query).await.unwrap();
        assert_eq!(
            paths(&found),
            HashSet::from(["/search/shared.mp3".into()]),
            "{query}"
        );
        assert!(found[0].has_multiple_audio_sources);
        assert_eq!(
            found[0].beatmaps,
            all[0]
                .beatmaps
                .iter()
                .filter(|map| map.audio_source_id == Some(found[0].audio_sources[0].id))
                .cloned()
                .collect::<Vec<_>>()
        );
        assert_eq!(found[1], all[1]); // Preserve metadata from other references.
        assert_eq!(
            found
                .iter()
                .map(|set| set.beatmap_set.id)
                .collect::<Vec<_>>(),
            all.iter().map(|set| set.beatmap_set.id).collect::<Vec<_>>()
        );
    }
    for query in [
        "missing",
        "hard insane",
        "starother",
        "rocketmissing",
        ".*",
        "rock%",
    ] {
        assert!(
            service
                .search_with_audio_sources(query)
                .await
                .unwrap()
                .is_empty(),
            "{query}"
        );
    }
    // Establish a read snapshot, commit a replacement through another pool, then read tags.
    let transaction = database.services.database.begin_read().await.unwrap();
    let projections = transaction
        .beatmap_sets()
        .search_difficulties()
        .await
        .unwrap();
    let metadata = transaction
        .beatmap_metadata()
        .search_metadata()
        .await
        .unwrap();
    assert!(!metadata.is_empty());
    let before = transaction
        .beatmap_sets()
        .all_with_audio_sources()
        .await
        .unwrap();
    other
        .osu_installations()
        .replace_snapshot(id, &snapshot("Replacement", "/search/new.mp3"))
        .await
        .unwrap();
    assert_eq!(
        transaction
            .tags()
            .matching_set_ids("rocket")
            .await
            .unwrap()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from([before[0].beatmap_set.id])
    );
    let ids: Vec<_> = projections
        .iter()
        .map(|map| map.audio_source_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let multiple = before
        .iter()
        .filter(|set| set.has_multiple_audio_sources)
        .map(|set| set.beatmap_set.id)
        .collect();
    assert_eq!(
        transaction
            .beatmap_sets()
            .for_audio_sources(&ids, &multiple)
            .await
            .unwrap(),
        before
    );
    let tags = transaction.tags().all_set_links().await.unwrap();
    assert!(
        tags.iter()
            .any(|tag| tag.beatmap_set_id == before[0].beatmap_set.id && tag.name == "rocket")
    );
    assert_eq!(
        transaction
            .beatmap_sets()
            .all_with_audio_sources()
            .await
            .unwrap(),
        before
    );
    transaction.commit().await.unwrap();
    assert!(
        service
            .search_with_audio_sources("rocket")
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        paths(
            &service
                .search_with_audio_sources("replacement")
                .await
                .unwrap()
        ),
        HashSet::from(["/search/new.mp3".into()])
    );
    database.osu_installations().delete(id).await.unwrap();
    large_search(database).await;
}

async fn large_search(database: &TestDatabase) {
    let id = register(database, "search-batches").await;
    let words = (0..70)
        .map(|n| format!("unique-word-{n:03}"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut imported = snapshot(&words, "/batch/first.mp3").remove(0);
    // One set straddles three ID chunks; retain every difficulty and global ordering.
    for n in 0..1001 {
        let mut part = snapshot(&words, &format!("/batch/{n}.mp3")).remove(0);
        let filename = format!("{n}.mp3");
        part.files[0].filename = Some(filename.clone());
        for map in &mut part.beatmaps {
            map.metadata.as_mut().unwrap().audio_file = Some(filename.clone());
        }
        imported.files.extend(part.files);
        imported.beatmaps.extend(part.beatmaps);
    }
    database
        .osu_installations()
        .replace_snapshot(id, &[imported])
        .await
        .unwrap();
    let all = database
        .beatmap_sets()
        .all_with_audio_sources()
        .await
        .unwrap();
    let expected: Vec<_> = all
        .into_iter()
        .filter(|set| set.beatmap_set.installation_id == id)
        .collect();
    assert_eq!(expected[0].audio_sources.len(), 1002);
    assert_eq!(
        database
            .beatmap_sets()
            .search_with_audio_sources(&words)
            .await
            .unwrap(),
        expected
    );
    assert_eq!(
        database
            .beatmap_sets()
            .search_with_audio_sources(&format!("{words} UNIQUE-WORD-001"))
            .await
            .unwrap(),
        expected
    );
    assert!(
        database
            .beatmap_sets()
            .search_with_audio_sources(&format!("{words} missing-last-word"))
            .await
            .unwrap()
            .is_empty()
    );
    database.osu_installations().delete(id).await.unwrap();
}
