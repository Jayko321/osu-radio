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
        meta.artist_unicode = Some("ЁЖ İSTANBUL ΟΣ".into());
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
    let mut empty = snapshot("Empty set", "/search/empty.mp3").remove(0);
    empty.beatmaps.clear();
    empty.files.clear();
    imported.push(empty);
    database
        .osu_installations()
        .replace_snapshot(id, &imported)
        .await
        .unwrap();
    let service = database.beatmap_sets();
    let all = service.all_with_audio_sources().await.unwrap();
    assert!(
        all.iter()
            .any(|set| set.beatmap_set.installation_id == id && set.audio_sources.is_empty())
    );
    for query in ["", "star", "roc HARD", "Other", "missing-last-word"] {
        let legacy = service.search_with_audio_sources(query).await.unwrap();
        assert_eq!(
            service.search_tracks(query).await.unwrap(),
            crate::beatmap_set::tracks::from_sets(legacy),
            "{query}"
        );
    }

    for query in ["", " \t\n\u{2003}"] {
        assert_eq!(service.search_with_audio_sources(query).await.unwrap(), all);
    }
    for query in [
        "roc",
        "ROCK",
        "orCH",
        "ёж",
        "звёз",
        "a+b%_&?#",
        "%",
        "_",
        "a+b",
        "&?#",
        "i\u{0307}stan",
        "İSTAN",
        "ΟΣ",
        "ος",
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
            all.iter()
                .filter(|set| !set.audio_sources.is_empty())
                .map(|set| set.beatmap_set.id)
                .collect::<Vec<_>>()
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
    assert_eq!(
        transaction
            .beatmap_sets()
            .matching_audio_sources("rocket")
            .await
            .unwrap()
            .into_iter()
            .collect::<HashSet<_>>(),
        ids.iter().copied().collect()
    );
    assert!(
        transaction
            .beatmap_sets()
            .matching_audio_sources("replacement")
            .await
            .unwrap()
            .is_empty()
    );
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
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        before
            .iter()
            .filter(|set| !set.audio_sources.is_empty())
            .collect::<Vec<_>>()
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
    text_search(database).await;
    numeric_search(database).await;
    temporal_search(database).await;
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
        database.beatmap_sets().search_tracks(&words).await.unwrap(),
        crate::beatmap_set::tracks::from_sets(
            database
                .beatmap_sets()
                .search_with_audio_sources(&words)
                .await
                .unwrap()
        )
    );
    assert_eq!(
        database.beatmap_sets().search_tracks(" ").await.unwrap(),
        crate::beatmap_set::tracks::from_sets(
            database
                .beatmap_sets()
                .all_with_audio_sources()
                .await
                .unwrap()
        )
    );
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
    cross_chunk_set_order(database, id, &words).await;
    database.osu_installations().delete(id).await.unwrap();
}

async fn cross_chunk_set_order(database: &TestDatabase, id: i32, words: &str) {
    // Reuse the existing low audio IDs in a later set after a new high-ID set.
    let mut later = snapshot(words, "/batch/first.mp3").remove(0);
    for n in 0..1001 {
        let mut part = snapshot(words, &format!("/batch/{n}.mp3")).remove(0);
        let filename = format!("{n}.mp3");
        part.files[0].filename = Some(filename.clone());
        for map in &mut part.beatmaps {
            map.metadata.as_mut().unwrap().audio_file = Some(filename.clone());
        }
        later.files.extend(part.files);
        later.beatmaps.extend(part.beatmaps);
    }
    let early = snapshot(words, "/batch/new-high.mp3").remove(0);
    database
        .osu_installations()
        .replace_snapshot(id, &[early, later])
        .await
        .unwrap();
    let legacy = database
        .beatmap_sets()
        .search_with_audio_sources(words)
        .await
        .unwrap();
    let expected_tracks = crate::beatmap_set::tracks::from_sets(legacy);
    assert_eq!(expected_tracks.len(), 1003);
    assert!(expected_tracks[0].audio_source_id > expected_tracks[1].audio_source_id);
    assert_eq!(
        database.beatmap_sets().search_tracks(words).await.unwrap(),
        expected_tracks
    );
}

#[allow(clippy::too_many_lines)]
async fn text_search(database: &TestDatabase) {
    use radio_core::import_types::RealmUser;
    let id = register(database, "search-text").await;
    let mut first = snapshot("freedom dive", "/text/first.mp3").remove(0);
    first.online_id = Some(12345);
    for map in &mut first.beatmaps {
        let meta = map.metadata.as_mut().unwrap();
        meta.artist = Some("xi".into());
        meta.artist_unicode = Some("ёж".into());
        meta.author = Some(RealmUser {
            online_id: None,
            username: Some("mapper name".into()),
            country_code: None,
        });
        meta.source = Some("game source".into());
        meta.tags = Some("rocket a+b%_&?#".into());
        meta.user_tags = vec!["two words".into(), "other tag".into()];
    }
    let mut alias = snapshot("another", "/text/first.mp3").remove(0);
    alias.online_id = Some(54321);
    for map in &mut alias.beatmaps {
        map.metadata.as_mut().unwrap().artist = Some("xion".into());
    }
    let mut second = snapshot(
        "12345 unknown=oops artist>=xi title= \"unfinished",
        "/text/second.mp3",
    )
    .remove(0);
    second.online_id = Some(99);
    for map in &mut second.beatmaps {
        let meta = map.metadata.as_mut().unwrap();
        meta.artist = Some("xion".into());
        meta.author = None;
        meta.tags = Some("xi".into());
        meta.source = Some("game".into());
    }
    database
        .osu_installations()
        .replace_snapshot(id, &[first, alias, second])
        .await
        .unwrap();
    let service = database.beatmap_sets();
    for (query, expected) in [
        ("easy hard", vec!["/text/first.mp3", "/text/second.mp3"]),
        ("artist=xi", vec!["/text/first.mp3", "/text/second.mp3"]),
        ("artist=\"xi\"", vec!["/text/first.mp3"]),
        ("artist=\"xi\"!", vec!["/text/first.mp3"]),
        (
            "artist=xi artist=on",
            vec!["/text/first.mp3", "/text/second.mp3"],
        ),
        ("artist!=xi", vec![]),
        ("artist!=\"xi\"", vec!["/text/second.mp3"]),
        ("artist=rocket", vec![]),
        ("tag=xi", vec!["/text/second.mp3"]),
        ("creator=mapper", vec!["/text/first.mp3"]),
        ("author=mapper mapper=name", vec!["/text/first.mp3"]),
        ("source=source", vec!["/text/first.mp3"]),
        ("mapper source", vec!["/text/first.mp3"]),
        ("\"freedom dive\"", vec!["/text/first.mp3"]),
        ("\"two words\"", vec!["/text/first.mp3"]),
        ("\"words other\"", vec![]),
        ("\"ёж\"", vec!["/text/first.mp3"]),
        ("a+b%_&?#", vec!["/text/first.mp3"]),
        ("unknown=oops", vec!["/text/second.mp3"]),
        ("unknown=missing", vec![]),
        ("artist>=xi", vec!["/text/second.mp3"]),
        ("title=", vec!["/text/second.mp3"]),
        ("\"unfinished", vec!["/text/second.mp3"]),
        ("12345", vec!["/text/first.mp3", "/text/second.mp3"]),
        ("12345 source=source", vec!["/text/first.mp3"]),
        ("0", vec![]),
        ("-12345", vec![]),
        ("12345 missing", vec![]),
    ] {
        let sets = service.search_with_audio_sources(query).await.unwrap();
        assert_eq!(
            paths(&sets),
            expected.into_iter().map(str::to_owned).collect(),
            "{query}"
        );
        assert_eq!(
            service.search_tracks(query).await.unwrap(),
            crate::beatmap_set::tracks::from_sets(sets),
            "{query}"
        );
    }
    let mut phrase = snapshot("phrase", "/text/phrase.mp3").remove(0);
    for map in &mut phrase.beatmaps {
        map.metadata.as_mut().unwrap().artist = Some("xi extra".into());
    }
    database
        .osu_installations()
        .replace_snapshot(id, &[phrase])
        .await
        .unwrap();
    assert_eq!(
        paths(
            &service
                .search_with_audio_sources("artist=\"xi\"")
                .await
                .unwrap()
        ),
        HashSet::from(["/text/phrase.mp3".into()])
    );
    assert!(
        service
            .search_with_audio_sources("artist=\"xi\"!")
            .await
            .unwrap()
            .is_empty()
    );
    database.osu_installations().delete(id).await.unwrap();
}

#[allow(clippy::too_many_lines)]
async fn numeric_search(database: &TestDatabase) {
    use crate::{AudioModsMode, AudioModsPatch, AudioModsProfile};
    let id = register(database, "search-numeric").await;
    let data = [
        (
            "split",
            [Some(100.0), Some(300.0)],
            [Some(5.099_999_999_999_999), Some(6.5)],
            Some(1_000),
        ),
        (
            "low",
            [Some(179.5), None],
            [Some(5.999), None],
            Some(61_001),
        ),
        ("high", [Some(180.5), None], [None, None], Some(3_600_000)),
        (
            "outside",
            [Some(179.49), Some(180.51)],
            [None, None],
            Some(1_001),
        ),
        ("interior", [Some(180.2), None], [None, None], Some(60_000)),
        ("unknown", [None, None], [None, None], None),
        (
            "cross",
            [Some(5.0), Some(200.0)],
            [Some(7.0), Some(4.1)],
            Some(4_000),
        ),
        ("length=bad bpm=oops", [None, None], [None, None], None),
    ];
    let mut imported = Vec::new();
    for (name, bpms, stars, _) in &data {
        let mut set = snapshot(name, &format!("/numeric/{name}.mp3")).remove(0);
        for (index, map) in set.beatmaps.iter_mut().enumerate() {
            map.bpm = bpms[index];
            map.stars = stars[index];
        }
        imported.push(set);
    }
    database
        .osu_installations()
        .replace_snapshot(id, &imported)
        .await
        .unwrap();
    let transaction = database.services.database.begin().await.unwrap();
    let mut source_ids = Vec::new();
    for (name, _, _, duration) in &data {
        let source = transaction
            .audio_sources()
            .find(&SourceType::Local(format!("/numeric/{name}.mp3")))
            .await
            .unwrap()
            .unwrap();
        transaction
            .audio_sources()
            .set_duration(source.id, *duration)
            .await
            .unwrap();
        source_ids.push(source.id);
    }
    transaction.commit().await.unwrap();
    let service = database.beatmap_sets();
    for (query, expected) in [
        ("bpm=180", vec!["low", "high", "interior"]),
        ("23 bpm=180", vec!["low", "high", "interior"]),
        ("title=split bpm>=180 bpm<=200", vec![]),
        ("title=split bpm>=100 bpm!=100 bpm<300", vec![]),
        ("title=low bpm=180", vec!["low"]),
        ("title=high bpm:180", vec!["high"]),
        ("title=outside bpm=180", vec![]),
        ("title=outside bpm!:180", vec!["outside"]),
        ("title=low bpm!=180", vec![]),
        ("title=interior bpm>180", vec!["interior"]),
        ("title=interior bpm<:180", vec![]),
        ("title=interior bpm>:180.2", vec!["interior"]),
        ("title=low stars=5.99", vec!["low"]),
        ("title=low stars>=6", vec![]),
        ("title=low stars>=5 sr<6", vec!["low"]),
        ("title=split stars>=6 sr<5.5", vec![]),
        ("title=split stars!=6.5", vec!["split"]),
        ("title=split stars=5.09", vec!["split"]),
        ("title=split stars=5.1", vec![]),
        ("title=unknown bpm!=180", vec![]),
        ("title=unknown stars!=5", vec![]),
        ("title=cross bpm=5 stars=4.1", vec!["cross"]),
        ("title=cross stars=4.1", vec!["cross"]),
        ("title=cross stars=4.09", vec![]),
        ("title=low 61.001", vec![]),
        ("title=low length=61.001", vec!["low"]),
        ("title=low length=1:01.001", vec!["low"]),
        ("title=low length=0:01:01.001", vec!["low"]),
        ("title=low length=61001ms", vec!["low"]),
        ("title=low length=61.001s", vec!["low"]),
        ("title=interior length=60", vec!["interior"]),
        ("title=interior length=1:00", vec!["interior"]),
        ("title=interior length=1m", vec!["interior"]),
        ("title=interior length=60000ms", vec!["interior"]),
        ("title=high length=1h", vec!["high"]),
        ("title=high length=60m", vec!["high"]),
        ("title=high length=1:00:00", vec!["high"]),
        ("title=split length=1000.1ms", vec![]),
        ("title=split length!=1000.1ms", vec!["split"]),
        ("title=split length<1000.1ms", vec!["split"]),
        ("title=split length>=1000.1ms", vec![]),
        ("title=outside length>1.0001s", vec!["outside"]),
        ("title=unknown length!=1s", vec![]),
        ("length=bad", vec!["length=bad bpm=oops"]),
        ("bpm=oops", vec!["length=bad bpm=oops"]),
        ("length=bad-no-match", vec![]),
    ] {
        let sets = service.search_with_audio_sources(query).await.unwrap();
        let expected = expected
            .into_iter()
            .map(|name| format!("/numeric/{name}.mp3"))
            .collect();
        assert_eq!(paths(&sets), expected, "{query}");
        assert_eq!(
            service.search_tracks(query).await.unwrap(),
            crate::beatmap_set::tracks::from_sets(sets),
            "{query}"
        );
    }
    let query = "title=low bpm=180 stars=5.99 length=61001ms";
    let before = paths(&service.search_with_audio_sources(query).await.unwrap());
    assert_eq!(before, HashSet::from(["/numeric/low.mp3".into()]));
    let enabled = database
        .audio_mods()
        .get()
        .await
        .unwrap()
        .individual_mods_enabled;
    database
        .audio_mods()
        .update(AudioModsPatch {
            individual_mods_enabled: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    database
        .audio_mods()
        .set_profile(
            source_ids[1],
            Some(AudioModsProfile {
                mode: AudioModsMode::Dt,
                speed_percent: 150,
                adjust_pitch: true,
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        paths(&service.search_with_audio_sources(query).await.unwrap()),
        before
    );
    database
        .audio_mods()
        .set_profile(source_ids[1], None)
        .await
        .unwrap();
    database
        .audio_mods()
        .update(AudioModsPatch {
            individual_mods_enabled: Some(enabled),
            ..Default::default()
        })
        .await
        .unwrap();
    database.osu_installations().delete(id).await.unwrap();
}

#[allow(clippy::too_many_lines, clippy::arithmetic_side_effects)]
async fn temporal_search(database: &TestDatabase) {
    use chrono::{TimeZone, Utc};
    let date = |year, month, day| {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0)
            .unwrap()
            .timestamp_millis()
    };
    let now = date(2025, 1, 15);
    let day = 86_400_000_i64;
    let id = register(database, "search-temporal").await;
    let mut sets = Vec::new();
    for (name, status, ranked, played) in [
        (
            "start",
            Some(BeatmapStatus::Ranked),
            Some(date(2024, 1, 1)),
            Some(now - 7 * day),
        ),
        (
            "end",
            Some(BeatmapStatus::Loved),
            Some(date(2025, 1, 1) - 1),
            Some(now - 7 * day + 1),
        ),
        (
            "next",
            Some(BeatmapStatus::Approved),
            Some(date(2025, 1, 1)),
            Some(now - 7 * day - 1),
        ),
        (
            "leap",
            Some(BeatmapStatus::Qualified),
            Some(date(2024, 2, 29)),
            None,
        ),
        ("pending", Some(BeatmapStatus::Pending), None, None),
        ("wip", Some(BeatmapStatus::Wip), None, None),
        ("graveyard", Some(BeatmapStatus::Graveyard), None, None),
        ("local", Some(BeatmapStatus::LocallyModified), None, None),
        ("unknown", None, None, None),
        (
            "mixed",
            Some(BeatmapStatus::Ranked),
            Some(date(2023, 1, 1)),
            Some(now - 30 * day),
        ),
        ("status=invalid", None, None, None),
    ] {
        let mut set = snapshot(name, &format!("/temporal/{name}.mp3")).remove(0);
        set.ranked_at_ms = ranked;
        set.submitted_at_ms = ranked;
        for map in &mut set.beatmaps {
            map.status = status;
            map.osu_last_played_at_ms = played;
            map.bpm = Some(180.0);
            map.stars = Some(5.0);
        }
        if name == "mixed" {
            set.beatmaps[1].status = None;
            set.beatmaps[1].osu_last_played_at_ms = Some(now - day);
        }
        sets.push(set);
    }
    let mut another = snapshot("mixed", "/temporal/mixed.mp3").remove(0);
    another.ranked_at_ms = Some(date(2025, 1, 1));
    sets.push(another);
    database
        .osu_installations()
        .replace_snapshot(id, &sets)
        .await
        .unwrap();
    let tx = database.services.database.begin().await.unwrap();
    tx.listening_history()
        .record_started(&SourceType::Local("/temporal/unknown.mp3".into()), 1, now)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let transaction = database.services.database.begin_read().await.unwrap();
    let mut ids = std::collections::HashMap::new();
    for set in &sets {
        let location = set.files[0]
            .file
            .as_ref()
            .unwrap()
            .resolved_path
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let source = transaction
            .audio_sources()
            .find(&SourceType::Local(location.clone()))
            .await
            .unwrap()
            .unwrap();
        ids.insert(location.clone(), source.id);
    }
    for (query, expected) in [
        ("title=start status=RaNkEd,LoVeD", true),
        ("title=end status=ranked,loved", true),
        ("title=mixed status!=ranked,loved", false),
        ("title=mixed status=none status=ranked", true),
        ("title=pending status=pending", true),
        ("title=wip status=wip", true),
        ("title=graveyard status=graveyard", true),
        ("title=local status=locallymodified", true),
        ("title=next status=approved", true),
        ("title=leap status=qualified", true),
        ("title=unknown status=none", true),
        ("title=start ranked=2024", true),
        ("title=end ranked=2024", true),
        ("title=next ranked=2024", false),
        ("title=next ranked>2024", true),
        ("title=end ranked>2024", false),
        ("title=leap ranked=2024-02", true),
        ("title=leap ranked=2024/02/29", true),
        ("title=leap created=2024.02.29", true),
        ("title=unknown ranked!=2024", false),
        ("title=mixed ranked>=2024 ranked<2024", false),
        ("title=mixed ranked>=2024 ranked<=2024", false),
        ("title=mixed ranked>=2024 ranked<2025", false),
        ("title=start ranked<2024", false),
        ("title=start ranked>=2024 ranked<=2024", true),
        ("title=next ranked!=2024 ranked>2024", true),
        ("title=start ranked<=2024 ranked>=2024", true),
        ("title=start lastplayed=7d", true),
        ("title=start lastplayed<604800.0001s", true),
        ("title=start lastplayed>=604800.0001s", false),
        ("title=start lastplayed!=604800.0001s", true),
        ("title=next lastplayed>604800.0001s", true),
        ("title=end lastplayed<7d", true),
        ("title=next lastplayed>7d", true),
        ("title=start lastplayed<7d", false),
        ("title=start lastplayed>7d", false),
        ("title=start lastplayed<=7d lastplayed>=168h", true),
        (
            "title=start lastplayed=10080m lastplayed=604800s lastplayed=1w lastplayed=7",
            true,
        ),
        ("title=mixed lastplayed>7d", false),
        ("title=unknown lastplayed!=7d", false),
        ("title=unknown played=false", true),
        ("title=start played=true", true),
        ("title=start played=yes played=1 played!=no", true),
        ("title=unknown played=no played=0", true),
        ("title=start played=true played=false", false),
        (
            "title=start status=ranked,loved submitted=2024 bpm=180 stars=5",
            true,
        ),
        ("title=status=invalid status=invalid", true),
        ("title=leap ranked=2023-02-29", false),
    ] {
        let name = query
            .split_whitespace()
            .next()
            .unwrap()
            .strip_prefix("title=")
            .unwrap();
        let location = format!("/temporal/{name}.mp3");
        let matched =
            crate::beatmap_set::BeatmapSetService::matching_audio_ids_at(&transaction, query, now)
                .await
                .unwrap();
        assert_eq!(
            matched.contains(ids.get(&location).unwrap()),
            expected,
            "{query}"
        );
    }
    transaction.commit().await.unwrap();
    let tracks = database
        .beatmap_sets()
        .search_tracks("title=start status=ranked,loved submitted=2024 bpm=180 stars=5")
        .await
        .unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].difficulties.len(), 2);
    database.osu_installations().delete(id).await.unwrap();
}
