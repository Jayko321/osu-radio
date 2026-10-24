use super::*;
use crate::{
    PlaybackAssignment, PlaybackCommand as Command, PlaybackMode as Mode, QueueEntryId,
    QueueItemRef as Item, QueuePlacement as Placement, QueueState, RepeatMode,
};
use std::collections::HashSet;

pub(super) async fn contracts(db: &TestDatabase, other: &TestDatabase, ids: [i32; 4]) {
    shuffle_and_restore(db, ids).await;
    shuffle_actually_permutes(db, ids).await;
    boundaries(db, ids).await;
    insertions_while_shuffled(db, ids).await;
    playlist_launch(db).await;
    modes_are_persistent(db, other, ids).await;
    db.queue().clear().await.unwrap();
    db.queue()
        .command(Command::SetShuffle { enabled: false })
        .await
        .unwrap();
}

pub(super) async fn cmd(db: &TestDatabase, command: Command) -> PlaybackAssignment {
    db.queue().command(command).await.unwrap()
}
pub(super) async fn shuffle(db: &TestDatabase, enabled: bool) -> PlaybackAssignment {
    cmd(db, Command::SetShuffle { enabled }).await
}
/// Fresh queue with the first entry playing and its launch confirmed.
pub(super) async fn start(db: &TestDatabase, ids: &[i32]) -> QueueState {
    db.queue().clear().await.unwrap();
    db.queue().append(ids.to_vec()).await.unwrap();
    cmd(
        db,
        Command::Play {
            audio_source_id: None,
        },
    )
    .await;
    super::queue::complete_checks(db).await;
    db.queue().get().await.unwrap()
}
pub(super) fn entry_ids(state: &QueueState) -> Vec<QueueEntryId> {
    state.entries.iter().map(|e| e.id).collect()
}
fn set(ids: &[QueueEntryId]) -> HashSet<QueueEntryId> {
    ids.iter().copied().collect()
}
/// Everything except the order of the tail is unchanged by a mode switch.
fn assert_same_playback(before: &QueueState, after: &QueueState) {
    assert_eq!(after.current_entry_id, before.current_entry_id);
    assert_eq!(after.playback_token, before.playback_token);
    assert_eq!(after.launch_source, before.launch_source);
    assert_eq!(after.history_len, before.history_len);
    assert_eq!(after.mode, before.mode);
    assert_eq!(after.waiting_for_check, before.waiting_for_check);
}

// A → [B] → C → D
async fn shuffle_and_restore(db: &TestDatabase, [a, b, c, x]: [i32; 4]) {
    start(db, &[a, b, c, x, a, b]).await;
    cmd(db, Command::Next).await;
    super::queue::complete_checks(db).await;
    let before = db.queue().get().await.unwrap();
    let original = entry_ids(&before);
    assert!(!before.shuffle && before.entries.iter().all(|e| e.shuffle_order.is_none()));

    shuffle(db, true).await;
    let shuffled = db.queue().get().await.unwrap();
    assert!(shuffled.shuffle);
    assert_eq!(shuffled.revision, before.revision.checked_add(1).unwrap());
    assert_same_playback(&before, &shuffled);
    assert_eq!(entry_ids(&shuffled)[..2], original[..2]);
    assert_eq!(set(&entry_ids(&shuffled)[2..]), set(&original[2..]));
    assert!(shuffled.entries.iter().all(|e| e.shuffle_order.is_some()));
    // Re-applying the same value changes nothing, not even the revision.
    assert_eq!(shuffle(db, true).await.revision, shuffled.revision);

    // Drag the last upcoming entry to the front and delete another; neither may change the original order.
    let tail = entry_ids(&shuffled)[2..].to_vec();
    db.queue().move_entry(tail[3], Some(tail[0])).await.unwrap();
    db.queue().remove_entry(tail[1]).await.unwrap();
    let edited = db.queue().get().await.unwrap();
    let expected = original[2..]
        .iter()
        .copied()
        .filter(|id| *id != tail[1])
        .collect::<Vec<_>>();
    assert_eq!(edited.entries.len(), 5);
    shuffle(db, false).await;
    let restored = db.queue().get().await.unwrap();
    assert!(!restored.shuffle && restored.entries.iter().all(|e| e.shuffle_order.is_none()));
    assert_eq!(entry_ids(&restored)[..2], original[..2]);
    assert_eq!(entry_ids(&restored)[2..], expected[..]);
    assert_eq!(restored.current_entry_id, before.current_entry_id);
    assert_eq!(restored.launch_source, before.launch_source);
    assert_eq!(restored.playback_token, before.playback_token);
    assert_eq!(shuffle(db, false).await.revision, restored.revision);
}

async fn shuffle_actually_permutes(db: &TestDatabase, [a, b, c, x]: [i32; 4]) {
    let before = start(db, &[a, b, c, x, a, b, c, x]).await;
    let original = entry_ids(&before);
    let mut differed = false;
    // 7! orders: twenty identity results in a row would be a broken shuffle, not luck.
    for _ in 0..20 {
        shuffle(db, true).await;
        let shuffled = db.queue().get().await.unwrap();
        assert_eq!(entry_ids(&shuffled)[0], original[0]);
        assert_eq!(set(&entry_ids(&shuffled)), set(&original));
        differed |= entry_ids(&shuffled) != original;
        shuffle(db, false).await;
        assert_eq!(entry_ids(&db.queue().get().await.unwrap()), original);
    }
    assert!(differed);
}

async fn boundaries(db: &TestDatabase, [a, b, c, x]: [i32; 4]) {
    // Empty queue.
    db.queue().clear().await.unwrap();
    assert!(shuffle(db, true).await.shuffle);
    assert!(db.queue().get().await.unwrap().entries.is_empty());
    db.queue().append(vec![a]).await.unwrap();
    assert!(
        db.queue().get().await.unwrap().entries[0]
            .shuffle_order
            .is_some(),
        "entries added to an empty shuffled queue get keys"
    );
    assert!(!shuffle(db, false).await.shuffle);

    // Stopped queue: the whole list is upcoming.
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, b, c]).await.unwrap();
    let before = db.queue().get().await.unwrap();
    assert_eq!(before.mode, Mode::Stopped);
    shuffle(db, true).await;
    let shuffled = db.queue().get().await.unwrap();
    assert_eq!(set(&entry_ids(&shuffled)), set(&entry_ids(&before)));
    assert_same_playback(&before, &shuffled);
    shuffle(db, false).await;
    assert_eq!(
        entry_ids(&db.queue().get().await.unwrap()),
        entry_ids(&before)
    );

    // Zero and one entry after the current one.
    for ids in [vec![a], vec![a, b]] {
        let before = start(db, &ids).await;
        shuffle(db, true).await;
        assert_eq!(
            entry_ids(&db.queue().get().await.unwrap()),
            entry_ids(&before)
        );
        shuffle(db, false).await;
        assert_eq!(
            entry_ids(&db.queue().get().await.unwrap()),
            entry_ids(&before)
        );
    }

    // An unavailable upcoming entry is shuffled like the rest and never becomes current.
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, b, c, x]).await.unwrap();
    cmd(
        db,
        Command::Play {
            audio_source_id: None,
        },
    )
    .await;
    let dead = db.queue().get().await.unwrap().entries[3].id;
    for check in db.queue().pending_checks().await.unwrap() {
        db.queue()
            .apply_check(check.clone(), check.entry_id != dead)
            .await
            .unwrap();
    }
    let before = db.queue().get().await.unwrap();
    shuffle(db, true).await;
    let shuffled = db.queue().get().await.unwrap();
    assert_eq!(set(&entry_ids(&shuffled)), set(&entry_ids(&before)));
    assert_eq!(shuffled.current_entry_id, before.current_entry_id);
    let mut visited = HashSet::new();
    loop {
        let assignment = cmd(db, Command::Next).await;
        let Some(id) = assignment.current_entry_id else {
            break;
        };
        assert_ne!(id, dead);
        visited.insert(id);
    }
    assert_eq!(visited, set(&entry_ids(&before)[1..3]));
}

async fn insertions_while_shuffled(db: &TestDatabase, [a, b, c, x]: [i32; 4]) {
    // Next and end insertions keep their place when the shuffle is switched off.
    let before = start(db, &[a, b, c, x]).await;
    let original = entry_ids(&before);
    shuffle(db, true).await;
    db.queue()
        .insert(vec![Item::Audio(b), Item::Audio(c)], Placement::Next)
        .await
        .unwrap();
    db.queue()
        .insert(vec![Item::Audio(x), Item::Audio(a)], Placement::End)
        .await
        .unwrap();
    let shuffled = db.queue().get().await.unwrap();
    let ids = entry_ids(&shuffled);
    assert_eq!(ids.len(), 8);
    assert_eq!(
        shuffled.audio_source_ids()[1..3],
        [b, c],
        "next batch is visible right away"
    );
    assert_eq!(
        shuffled.audio_source_ids()[6..],
        [x, a],
        "end batch is visible right away"
    );
    shuffle(db, false).await;
    let restored = db.queue().get().await.unwrap();
    let expected = [ids[0], ids[1], ids[2]]
        .into_iter()
        .chain(original[1..].iter().copied())
        .chain([ids[6], ids[7]])
        .collect::<Vec<_>>();
    assert_eq!(entry_ids(&restored), expected);

    // Play from the library puts the new entry right after the current one and starts it.
    let before = start(db, &[a, b, c]).await;
    let original = entry_ids(&before);
    shuffle(db, true).await;
    let played = cmd(
        db,
        Command::Play {
            audio_source_id: Some(x),
        },
    )
    .await;
    let current = played.current_entry_id.unwrap();
    let state = db.queue().get().await.unwrap();
    assert_eq!(state.entries[1].id, current);
    assert_eq!(played.current_audio_source_id, Some(x));
    shuffle(db, false).await;
    let restored = db.queue().get().await.unwrap();
    assert_eq!(
        entry_ids(&restored),
        [original[0], current, original[1], original[2]]
    );
    assert_eq!(restored.current_entry_id, Some(current));
}

/// Playlist items resolve by hash, so each track needs its own unique beatmap hash.
#[allow(clippy::too_many_lines)] // One fixture playlist serves every launch scenario.
async fn playlist_launch(db: &TestDatabase) {
    let installation = register(db, "queue-modes").await;
    let mut sets = Vec::new();
    for title in ["PA", "PB", "PC", "PD"] {
        let mut set = snapshot(title, &format!("/queue-modes/{title}"));
        for map in &mut set[0].beatmaps {
            map.hash = Some(format!(
                "modes-{title}-{}",
                map.difficulty_name.as_deref().unwrap()
            ));
        }
        sets.append(&mut set);
    }
    db.osu_installations()
        .replace_snapshot(installation, &sets)
        .await
        .unwrap();
    let tracks = db.beatmap_sets().search_tracks("").await.unwrap();
    let found = ["PA", "PB", "PC", "PD"].map(|title| {
        tracks
            .iter()
            .find(|t| t.title.as_deref() == Some(title))
            .unwrap()
    });
    let ids = found.map(|t| t.audio_source_id);
    let maps = found.map(|t| t.difficulties[0].beatmap_id);
    let playlist = db.playlists().create("T02 modes").await.unwrap();
    let playlist = db.playlists().add_items(playlist.id, &maps).await.unwrap();
    assert_eq!(playlist.items.len(), 4);
    let [a, b, c, x] = ids;
    let item_c = playlist.items[2].id;

    // Without shuffle the launch is unchanged.
    db.queue().clear().await.unwrap();
    let plain = db
        .playlists()
        .play(playlist.id, Some(item_c))
        .await
        .unwrap();
    assert_eq!(plain.current_audio_source_id, Some(c));
    assert_eq!(
        db.queue().get().await.unwrap().audio_source_ids(),
        [a, b, c, x]
    );

    // Explicit start: it plays first, the other three follow in any order, switching off restores them.
    db.queue().clear().await.unwrap();
    shuffle(db, true).await;
    let started = db
        .playlists()
        .play(playlist.id, Some(item_c))
        .await
        .unwrap();
    assert_eq!(started.current_audio_source_id, Some(c));
    assert_eq!(started.mode, Mode::Playing);
    let state = db.queue().get().await.unwrap();
    assert_eq!(state.current_index(), Some(0));
    assert_eq!(
        state.audio_source_ids()[1..]
            .iter()
            .copied()
            .collect::<HashSet<_>>(),
        HashSet::from([a, b, x])
    );
    shuffle(db, false).await;
    assert_eq!(
        db.queue().get().await.unwrap().audio_source_ids(),
        [c, a, b, x]
    );

    // No explicit start: the whole playlist is shuffled and its first entry starts.
    db.queue().clear().await.unwrap();
    shuffle(db, true).await;
    let started = db.playlists().play(playlist.id, None).await.unwrap();
    let state = db.queue().get().await.unwrap();
    assert_eq!(state.current_index(), Some(0));
    assert_eq!(
        started.current_audio_source_id,
        state.audio_source_ids().first().copied()
    );
    assert_eq!(
        state
            .audio_source_ids()
            .iter()
            .copied()
            .collect::<HashSet<_>>(),
        HashSet::from(ids)
    );
    let first = state.audio_source_ids()[0];
    shuffle(db, false).await;
    let expected = std::iter::once(first)
        .chain(ids.into_iter().filter(|id| *id != first))
        .collect::<Vec<_>>();
    assert_eq!(db.queue().get().await.unwrap().audio_source_ids(), expected);

    // Earlier entries stay put and keep consistent keys.
    start(db, &[a, b]).await;
    cmd(db, Command::Next).await;
    shuffle(db, true).await;
    let before = db.queue().get().await.unwrap();
    db.playlists()
        .play(playlist.id, Some(item_c))
        .await
        .unwrap();
    let state = db.queue().get().await.unwrap();
    assert_eq!(state.audio_source_ids()[..2], [a, b]);
    assert_eq!(state.entries[..2], before.entries[..2]);
    assert_eq!(state.audio_source_ids()[2], c);
    shuffle(db, false).await;
    assert_eq!(
        db.queue().get().await.unwrap().audio_source_ids(),
        [a, b, c, a, b, x]
    );

    // Appending a playlist keeps its order at the end.
    start(db, &[a, b, c]).await;
    shuffle(db, true).await;
    db.playlists().append_to_queue(playlist.id).await.unwrap();
    assert_eq!(
        db.queue().get().await.unwrap().audio_source_ids()[3..],
        [a, b, c, x]
    );
    shuffle(db, false).await;
    assert_eq!(
        db.queue().get().await.unwrap().audio_source_ids(),
        [a, b, c, a, b, c, x]
    );
    db.queue().clear().await.unwrap();
    db.playlists().delete(playlist.id).await.unwrap();
    db.osu_installations().delete(installation).await.unwrap();
}

async fn modes_are_persistent(db: &TestDatabase, other: &TestDatabase, [a, b, c, _]: [i32; 4]) {
    let before = start(db, &[a, b, c]).await;
    for mode in [RepeatMode::All, RepeatMode::One, RepeatMode::Off] {
        let changed = cmd(db, Command::SetRepeat(mode)).await;
        assert_eq!(changed.repeat, mode);
        assert_eq!(changed.playback_token, before.playback_token);
        assert_eq!(db.queue().playback().await.unwrap().repeat, mode);
        assert_eq!(db.queue().snapshot().await.unwrap().repeat, mode);
        assert_eq!(
            cmd(db, Command::SetRepeat(mode)).await.revision,
            changed.revision
        );
    }
    cmd(db, Command::SetRepeat(RepeatMode::All)).await;
    shuffle(db, true).await;
    let state = db.queue().get().await.unwrap();
    assert_same_playback(&before, &state);
    assert_eq!((state.shuffle, state.repeat), (true, RepeatMode::All));
    assert_eq!(other.queue().get().await.unwrap(), state);
    let snapshot = db.queue().snapshot().await.unwrap();
    assert_eq!((snapshot.shuffle, snapshot.repeat), (true, RepeatMode::All));

    db.queue().recover().await.unwrap();
    let recovered = db.queue().get().await.unwrap();
    assert_eq!(
        (recovered.shuffle, recovered.repeat),
        (true, RepeatMode::All)
    );
    db.queue().clear().await.unwrap();
    let cleared = db.queue().playback().await.unwrap();
    assert!(cleared.shuffle);
    assert_eq!(cleared.repeat, RepeatMode::All);
    assert!(db.queue().get().await.unwrap().entries.is_empty());
    shuffle(db, false).await;
    cmd(db, Command::SetRepeat(RepeatMode::Off)).await;
}
