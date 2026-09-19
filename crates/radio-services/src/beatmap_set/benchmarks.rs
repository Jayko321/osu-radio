//! Opt-in synthetic measurement; never uses the configured application database.
use super::*;
use sea_orm::ConnectionTrait;
use std::time::Instant;

#[tokio::test]
#[ignore = "synthetic bulk read/matching timing; PostgreSQL requires a disposable RADIO_DB_TEST_POSTGRES_URL"]
async fn synthetic_search_timings() {
    #[cfg(feature = "sqlite")]
    let directory = tempfile::tempdir().unwrap();
    #[cfg(feature = "sqlite")]
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("search.sqlite").display()
    );
    #[cfg(feature = "postgres")]
    let url = {
        let url = std::env::var("RADIO_DB_TEST_POSTGRES_URL").unwrap();
        assert!(
            url.rsplit('/')
                .next()
                .unwrap()
                .starts_with("radio_db_test_")
        );
        url
    };
    let services = crate::Services::connect(&url).await.unwrap();
    let mut options = sea_orm::ConnectOptions::new(url);
    options.sqlx_logging(false);
    let connection = sea_orm::Database::connect(options).await.unwrap();
    for size in [1_000, 10_000, 50_000] {
        services.reset().await.unwrap();
        seed(&connection, size).await;
        let words = vec!["roc".to_owned(), "hard".to_owned()];
        let mut timings = Vec::new();
        for sample in 0..6 {
            let start = Instant::now();
            let transaction = services.database.begin_read().await.unwrap();
            let metadata = transaction
                .beatmap_metadata()
                .search_metadata()
                .await
                .unwrap();
            let maps = transaction
                .beatmap_sets()
                .search_difficulties()
                .await
                .unwrap();
            let mut tags = Vec::new();
            for word in &words {
                tags.push(transaction.tags().matching_set_ids(word).await.unwrap());
            }
            let read = start.elapsed();
            let start = Instant::now();
            let (ids, multiple) = matching_audio(metadata, maps, &tags, &words);
            let matching = start.elapsed();
            let start = Instant::now();
            let found = transaction
                .beatmap_sets()
                .for_audio_sources(&ids, &multiple)
                .await
                .unwrap();
            transaction.commit().await.unwrap();
            let load = start.elapsed();
            assert_eq!(found.len(), usize::try_from(size).unwrap());
            assert!(
                found
                    .iter()
                    .all(|set| set.audio_sources.len() == 1 && set.beatmaps.len() == 2)
            );
            if sample != 0 {
                timings.push((read, matching, load));
            }
        }
        let mut reads: Vec<_> = timings.iter().map(|t| t.0).collect();
        let mut matches: Vec<_> = timings.iter().map(|t| t.1).collect();
        let mut loads: Vec<_> = timings.iter().map(|t| t.2).collect();
        reads.sort();
        matches.sort();
        loads.sort();
        eprintln!(
            "sets={size}, read={:?}, match={:?}, load={:?}",
            reads[2], matches[2], loads[2]
        );
    }
    connection.close().await.unwrap();
}

async fn seed(connection: &sea_orm::DatabaseConnection, size: i32) {
    // Fixed-width-ish text and two independent audio per set; setup excluded from timings.
    for statement in [
        "INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (1,1,'lazer','\"/synthetic\"','\"/synthetic/client.realm\"')".to_owned(),
        format!("WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM numbers WHERE n<{size}) INSERT INTO beatmap_sets(id,installation_id) SELECT n,1 FROM numbers"),
        "INSERT INTO beatmap_metadata(hash,title,title_unicode,artist,artist_unicode) SELECT CAST(id AS TEXT),'Starlight orchestra','ЗВЁЗДЫ','Synthetic artist','ЁЖ' FROM beatmap_sets".to_owned(),
        "INSERT INTO audio_sources(id,kind,location) SELECT id*2-1,'local','/audio/' || CAST(id AS TEXT) || '.mp3' FROM beatmap_sets UNION ALL SELECT id*2,'local','/audio/' || CAST(id AS TEXT) || '-other.mp3' FROM beatmap_sets".to_owned(),
        "INSERT INTO beatmaps(id,beatmap_set_id,metadata_hash,audio_source_id,difficulty_name) SELECT s.id*4-d.n,s.id,CAST(s.id AS TEXT),s.id*2-CASE WHEN d.n<2 THEN 1 ELSE 0 END,CASE d.n WHEN 0 THEN 'Easy' WHEN 1 THEN 'Hard' WHEN 2 THEN 'Insane' ELSE 'Extra' END FROM beatmap_sets s CROSS JOIN (SELECT 0 AS n UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) d".to_owned(),
        "INSERT INTO tags(id,name) VALUES (1,'rocket'),(2,'piano')".to_owned(),
        "INSERT INTO beatmap_set_tags(beatmap_set_id,tag_id) SELECT s.id,t.id FROM beatmap_sets s CROSS JOIN tags t".to_owned(),
    ] {
        connection.execute_unprepared(&statement).await.unwrap();
    }
}

#[cfg(feature = "sqlite")]
fn legacy_filter_audio(
    mut sets: Vec<BeatmapSetWithAudio>,
    tags: &ReferenceTags,
    words: &[String],
) -> Vec<BeatmapSetWithAudio> {
    let mut set_matches: HashMap<i32, Vec<bool>> = HashMap::new();
    match tags {
        ReferenceTags::Names(tags) => {
            for tag in tags {
                let matches = set_matches
                    .entry(tag.beatmap_set_id)
                    .or_insert_with(|| vec![false; words.len()]);
                match_text(matches, words, &tag.name);
            }
        }
        ReferenceTags::Ids(tags) => {
            for (index, ids) in tags.iter().enumerate() {
                for id in ids {
                    let matches = set_matches
                        .entry(*id)
                        .or_insert_with(|| vec![false; words.len()]);
                    matches[index] = true;
                }
            }
        }
    }
    let mut audio_matches: HashMap<i32, Vec<bool>> = HashMap::new();
    for set in &sets {
        let tagged = set_matches.get(&set.beatmap_set.id);
        for map in &set.beatmaps {
            let Some(audio) = map.audio_source_id else {
                continue;
            };
            let matches = audio_matches
                .entry(audio)
                .or_insert_with(|| vec![false; words.len()]);
            if let Some(tagged) = tagged {
                for (matched, tag_match) in matches.iter_mut().zip(tagged) {
                    *matched |= tag_match;
                }
            }
            for text in [
                &map.title,
                &map.title_unicode,
                &map.artist,
                &map.artist_unicode,
                &map.difficulty_name,
            ]
            .into_iter()
            .flatten()
            {
                match_text(matches, words, &text.to_lowercase());
            }
        }
    }
    let matching: HashSet<_> = audio_matches
        .into_iter()
        .filter(|(_, matches)| matches.iter().all(|matched| *matched))
        .map(|(id, _)| id)
        .collect();
    // Keep every reference of matched audio, preserving the unfiltered representative and cover.
    for set in &mut sets {
        set.audio_sources
            .retain(|audio| matching.contains(&audio.id));
        set.beatmaps
            .retain(|map| map.audio_source_id.is_some_and(|id| matching.contains(&id)));
    }
    sets.retain(|set| !set.audio_sources.is_empty());
    sets
}

/// Run only against a user-created consistent SQLite backup. Never migrates or resets it.
#[cfg(feature = "sqlite")]
#[tokio::test]
#[ignore = "requires RADIO_SEARCH_BENCH_DB pointing to a consistent backup"]
async fn snapshot_search_timings() {
    let path = std::env::var("RADIO_SEARCH_BENCH_DB").unwrap();
    let services = crate::Services::connect(&path).await.unwrap();
    for query in ["rock", "rock hard", "星", "does-not-exist-zzzz", ""] {
        let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut reference = None;
        for stage in 0..3 {
            let mut timings = Vec::new();
            for sample in 0..6 {
                let start = Instant::now();
                let tx = services.database.begin_read().await.unwrap();
                let (found, read, matching, load) = if words.is_empty() {
                    let found = tx.beatmap_sets().all_with_audio_sources().await.unwrap();
                    (
                        found,
                        start.elapsed(),
                        std::time::Duration::ZERO,
                        std::time::Duration::ZERO,
                    )
                } else if stage < 2 {
                    let sets = tx.beatmap_sets().all_with_audio_sources().await.unwrap();
                    let tags = if stage == 0 {
                        ReferenceTags::Names(tx.tags().all_set_links().await.unwrap())
                    } else {
                        let mut tags = Vec::new();
                        for word in &words {
                            tags.push(tx.tags().matching_set_ids(word).await.unwrap());
                        }
                        ReferenceTags::Ids(tags)
                    };
                    let read = start.elapsed();
                    let start = Instant::now();
                    let found = legacy_filter_audio(sets, &tags, &words);
                    (found, read, start.elapsed(), std::time::Duration::ZERO)
                } else {
                    let metadata = tx.beatmap_metadata().search_metadata().await.unwrap();
                    let maps = tx.beatmap_sets().search_difficulties().await.unwrap();
                    let mut tags = Vec::new();
                    for word in &words {
                        tags.push(tx.tags().matching_set_ids(word).await.unwrap());
                    }
                    let read = start.elapsed();
                    let start = Instant::now();
                    let (ids, multiple) = matching_audio(metadata, maps, &tags, &words);
                    let matching = start.elapsed();
                    let start = Instant::now();
                    let found = tx
                        .beatmap_sets()
                        .for_audio_sources(&ids, &multiple)
                        .await
                        .unwrap();
                    (found, read, matching, start.elapsed())
                };
                tx.commit().await.unwrap();
                if let Some(expected) = &reference {
                    assert_eq!(&found, expected, "{stage}: {query}");
                } else {
                    reference = Some(found);
                }
                if sample != 0 {
                    timings.push((
                        read.as_secs_f64() * 1000.,
                        matching.as_secs_f64() * 1000.,
                        load.as_secs_f64() * 1000.,
                    ));
                }
            }
            let mut read: Vec<_> = timings.iter().map(|t| t.0).collect();
            let mut matching: Vec<_> = timings.iter().map(|t| t.1).collect();
            let mut load: Vec<_> = timings.iter().map(|t| t.2).collect();
            read.sort_by(f64::total_cmp);
            matching.sort_by(f64::total_cmp);
            load.sort_by(f64::total_cmp);
            eprintln!(
                "stage={stage} q={query:?} read_ms={:.3} match_ms={:.3} load_ms={:.3}",
                read[2], matching[2], load[2]
            );
        }
    }
}

#[cfg(feature = "sqlite")]
enum ReferenceTags {
    Names(Vec<crate::model::SetTag>),
    Ids(Vec<Vec<i32>>),
}
