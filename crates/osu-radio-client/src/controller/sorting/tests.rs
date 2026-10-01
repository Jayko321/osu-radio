use super::*;
use crate::controller::{AppCommand, MediaJob, MediaTicket, PlaylistAction};

fn track(id: i32, title: &str, artist: &str, date: Option<i64>) -> Track {
    let mut track = super::super::tests::track(id);
    track.title = title.into();
    track.artist = artist.into();
    track.last_played_at_ms = date;
    track
}

fn ids(tracks: &[Track]) -> Vec<i32> {
    tracks.iter().map(|track| track.audio_source_id).collect()
}

#[test]
fn every_sort_uses_unicode_case_secondary_text_dates_and_stable_ids() {
    let source = vec![
        track(1, "Б", "Я", Some(20)),
        track(2, "а", "Б", Some(20)),
        track(3, "б", "а", Some(10)),
        track(4, "А", "б", Some(20)),
        track(5, "", "", None),
    ];
    for (sort, expected) in [
        (TrackSort::TitleAsc, vec![5, 2, 4, 3, 1]),
        (TrackSort::ArtistAsc, vec![5, 3, 2, 4, 1]),
        (TrackSort::RecentlyPlayed, vec![2, 4, 1, 3, 5]),
    ] {
        let mut tracks = source.clone();
        sort.sort_tracks(&mut tracks);
        assert_eq!(ids(&tracks), expected);
        sort.sort_tracks(&mut []);
    }
    let row: crate::models::LibraryTrack = serde_json::from_value(serde_json::json!({
        "audio_source_id": 9, "title":" ", "title_unicode":"曲", "artist":"", "artist_unicode":"А", "cover_beatmap_id":null, "difficulties":[]
    })).unwrap();
    let row = Track::from(row);
    assert_eq!(
        (&*row.title, &*row.artist, row.last_played_at_ms),
        ("曲", "А", None)
    );
}

#[test]
fn reordering_preserves_selection_durations_artwork_jobs_and_media_generation() {
    let (mut state, updates) = super::super::tests::controller();
    let mut first = track(1, "Zulu", "a", None);
    first.duration = Some(std::time::Duration::from_secs(42));
    state.replace_tracks(vec![first, track(2, "Alpha", "z", Some(20))], true);
    state.command(AppCommand::SelectTrack(Some(1)));
    let ticket = MediaTicket {
        generation: state.generation,
        audio_id: 1,
        serial: 7,
    };
    state.jobs.insert(
        7,
        MediaJob {
            ticket,
            cover: Some(101),
            decoding: true,
            artwork_pending: true,
            duration_pending: true,
            tasks: Vec::new(),
        },
    );
    state.queue.push_back((2, true));
    state.durations.insert(1, None);
    state.unavailable.insert(102);
    let generation = state.generation;
    updates.lock().unwrap().clear();
    for sort in [
        TrackSort::ArtistAsc,
        TrackSort::RecentlyPlayed,
        TrackSort::TitleAsc,
    ] {
        state.command(AppCommand::SetTrackSort(sort));
        assert_eq!(state.selected, Some(1));
        assert_eq!(
            state.track(1).unwrap().duration,
            Some(std::time::Duration::from_secs(42))
        );
        assert_eq!(state.generation, generation);
        assert_eq!(state.jobs.get(&7).unwrap().ticket, ticket);
        assert_eq!(state.queue.front(), Some(&(2, true)));
        assert!(state.durations.contains_key(&1) && state.unavailable.contains(&102));
        assert_eq!(state.track_indices.len(), 2);
    }
    assert!(
        updates
            .lock()
            .unwrap()
            .iter()
            .all(|update| !matches!(update, AppUpdate::TracksReplaced { .. }))
    );
    assert!(
        updates
            .lock()
            .unwrap()
            .iter()
            .any(|update| matches!(update, AppUpdate::TracksReordered(_)))
    );
    let count = updates.lock().unwrap().len();
    state.command(AppCommand::SetTrackSort(TrackSort::TitleAsc));
    assert_eq!(updates.lock().unwrap().len(), count);
}

#[test]
fn incoming_search_results_keep_sort_and_newer_known_dates() {
    let (mut state, _) = super::super::tests::controller();
    state.command(AppCommand::SetTrackSort(TrackSort::RecentlyPlayed));
    state.replace_tracks(
        vec![track(1, "A", "A", Some(30)), track(2, "B", "B", Some(40))],
        true,
    );
    state.library_request = 2;
    state.complete(super::super::Completed::Library {
        request: 1,
        invalidate_artwork: true,
        result: Ok(vec![track(3, "Z", "Z", Some(100))]),
    });
    assert_eq!(ids(&state.tracks), [2, 1]);
    state.library_query = "search".into();
    state.complete(super::super::Completed::Library {
        request: 2,
        invalidate_artwork: true,
        result: Ok(vec![track(2, "B", "B", None), track(1, "A", "A", Some(1))]),
    });
    assert_eq!(ids(&state.tracks), [2, 1]);
    assert_eq!(state.tracks.first().unwrap().last_played_at_ms, Some(40));
    state.replace_tracks(Vec::new(), true);
    state.replace_tracks(
        vec![track(1, "A", "A", None), track(2, "B", "B", None)],
        true,
    );
    assert_eq!(ids(&state.tracks), [2, 1]);
    state.command(AppCommand::Playlist(PlaylistAction::Select(None)));
    assert_eq!(state.track_sort, TrackSort::RecentlyPlayed);
}
