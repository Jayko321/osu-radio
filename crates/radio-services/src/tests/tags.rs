use super::*;

#[allow(clippy::too_many_lines)]
pub(super) async fn contracts(database: &TestDatabase, other: &TestDatabase) {
    let first = register(database, "tags-lazer").await;
    let second = database
        .osu_installations()
        .register(
            &OsuMarker {
                kind: OsuKind::Stable,
                ..marker("tags-stable")
            },
            None,
        )
        .await
        .unwrap()
        .into_installation()
        .id;
    let mut imported = snapshot("Tags", "/audio/tags.mp3");
    let easy = imported[0].beatmaps[0].metadata.as_mut().unwrap();
    easy.tags = Some("  Rock\tjapanese\nROCK\u{2003}ÄPFEL ЁЖ İSTANBUL  ".into());
    easy.user_tags = vec![
        " Piano Only ".into(),
        "ROCK".into(),
        String::new(),
        " \t ".into(),
    ];
    let hard = imported[0].beatmaps[1].metadata.as_mut().unwrap();
    hard.tags = Some("rock zeta".into());
    hard.user_tags = vec!["piano only".into(), "  Multi  Word  ".into()];
    database
        .osu_installations()
        .replace_snapshot(first, &imported)
        .await
        .unwrap();
    let all = database.tags().all().await.unwrap();
    assert_eq!(
        all.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        [
            "i\u{307}stanbul",
            "japanese",
            "multi  word",
            "piano only",
            "rock",
            "zeta",
            "äpfel",
            "ёж"
        ]
    );
    assert!(all.iter().all(|tag| tag.id > 0));
    for tag in &all {
        assert_eq!(
            database.tags().get(tag.id).await.unwrap(),
            Some(tag.clone())
        );
    }
    assert_eq!(database.tags().get(i32::MAX).await.unwrap(), None);
    assert!(database.tags().for_set(i32::MAX).await.unwrap().is_empty());
    let sets = database
        .beatmap_sets()
        .for_installation(first)
        .await
        .unwrap();
    assert_eq!(database.tags().for_set(sets[0].id).await.unwrap(), all);
    let maps = database.beatmaps().for_set(sets[0].id).await.unwrap();
    assert_eq!(maps[0].metadata_hash, maps[1].metadata_hash);
    // Reimport retains IDs even when this installation is their only owner.
    database
        .osu_installations()
        .replace_snapshot(first, &imported)
        .await
        .unwrap();
    assert_eq!(database.tags().all().await.unwrap(), all);
    let sets = database
        .beatmap_sets()
        .for_installation(first)
        .await
        .unwrap();
    assert_eq!(database.tags().for_set(sets[0].id).await.unwrap(), all);
    let mut stable = snapshot("Other metadata", "/audio/stable.mp3");
    stable[0].source = OsuKind::Stable;
    for map in &mut stable[0].beatmaps {
        let metadata = map.metadata.as_mut().unwrap();
        metadata.tags = Some("ROCK".into());
        metadata.user_tags = vec!["Rock".into(), "rock".into()];
    }
    // A second set and source must use the same global identity.
    stable.push(stable[0].clone());
    other
        .osu_installations()
        .replace_snapshot(second, &stable)
        .await
        .unwrap();
    let rock = all.iter().find(|tag| tag.name == "rock").unwrap().clone();
    for set in other.beatmap_sets().for_installation(second).await.unwrap() {
        assert_eq!(
            other.tags().for_set(set.id).await.unwrap(),
            std::slice::from_ref(&rock)
        );
    }
    assert_eq!(database.tags().all().await.unwrap(), all);
    database.osu_installations().delete(first).await.unwrap();
    assert_eq!(database.tags().all().await.unwrap(), [rock]);
    other
        .osu_installations()
        .replace_snapshot(second, &[])
        .await
        .unwrap();
    assert!(database.tags().all().await.unwrap().is_empty());
    other
        .osu_installations()
        .replace_snapshot(second, &[])
        .await
        .unwrap();
    // Sets with missing metadata and only empty names have no links or tag rows.
    stable.truncate(1);
    stable[0].beatmaps[0].metadata = None;
    let metadata = stable[0].beatmaps[1].metadata.as_mut().unwrap();
    metadata.tags = Some("\n\t ".into());
    metadata.user_tags = vec![String::new(), " \u{2003} ".into()];
    other
        .osu_installations()
        .replace_snapshot(second, &stable)
        .await
        .unwrap();
    assert!(database.tags().all().await.unwrap().is_empty());
    other.osu_installations().delete(second).await.unwrap();
}
