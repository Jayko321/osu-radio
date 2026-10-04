use super::*;
use crate::{controller::AppCommand, models::LibraryTrack};
use std::time::Duration;

fn track(id: i32, ordinary: &str, unicode: &str) -> Track {
    Track::from(LibraryTrack {
        audio_source_id: id,
        last_played_at_ms: None,
        volume_percent: None,
        title: Some(ordinary.into()),
        title_unicode: Some(unicode.into()),
        artist: Some("Artist".into()),
        artist_unicode: Some("演奏者".into()),
        cover_beatmap_id: Some(id.saturating_add(100)),
        difficulties: Vec::new(),
    })
}

#[test]
fn four_combinations_and_blank_fallbacks_preserve_suffix_and_media() {
    let mut row = track(1, "Song", "曲");
    row.subtitle_suffix = " | Hard · Unavailable".into();
    row.duration = Some(Duration::from_secs(42));
    for use_unicode_titles in [false, true] {
        for use_unicode_artists in [false, true] {
            row.apply_name_preferences(TrackNamePreferences {
                use_unicode_titles,
                use_unicode_artists,
            });
            assert_eq!(row.title, if use_unicode_titles { "曲" } else { "Song" });
            assert_eq!(
                row.artist,
                if use_unicode_artists {
                    "演奏者"
                } else {
                    "Artist"
                }
            );
            assert_eq!(row.subtitle, format!("{} | Hard · Unavailable", row.artist));
            assert_eq!(row.duration, Some(Duration::from_secs(42)));
            assert_eq!(row.cover_beatmap_id, Some(101));
        }
    }
    row.title_original = Some(" \t".into());
    row.artist_unicode = Some("\n".into());
    for enabled in [false, true] {
        row.apply_name_preferences(TrackNamePreferences {
            use_unicode_titles: enabled,
            use_unicode_artists: enabled,
        });
        assert_eq!((&*row.title, &*row.artist), ("曲", "Artist"));
    }
    row.title_unicode = None;
    row.artist_original = None;
    row.apply_name_preferences(TrackNamePreferences::default());
    assert_eq!(
        (&*row.title, &*row.artist),
        ("Unknown title", "Unknown artist")
    );
}

#[test]
fn rapid_changes_reorder_library_keep_queue_order_and_use_current_values_for_late_rows() {
    let (mut state, updates) = super::super::tests::controller();
    let mut selected = track(1, "Zulu", "Alpha");
    selected.duration = Some(Duration::from_secs(42));
    state.replace_tracks(vec![selected.clone(), track(2, "Alpha", "Zulu")], false);
    state.command(AppCommand::SelectTrack(Some(1)));
    state.current_track = Some(selected);
    state.upcoming.view.tracks = vec![track(2, "Alpha", "Zulu"), track(1, "Zulu", "Alpha")];
    let generation = state.generation;
    for preferences in [
        TrackNamePreferences {
            use_unicode_titles: true,
            use_unicode_artists: false,
        },
        TrackNamePreferences {
            use_unicode_titles: true,
            use_unicode_artists: true,
        },
        TrackNamePreferences {
            use_unicode_titles: false,
            use_unicode_artists: true,
        },
        TrackNamePreferences {
            use_unicode_titles: true,
            use_unicode_artists: true,
        },
    ] {
        state.command(AppCommand::SetTrackNamePreferences(preferences));
        assert_eq!(state.selected, Some(1));
        assert_eq!(state.generation, generation);
        assert_eq!(
            state.current_track.as_ref().unwrap().duration,
            Some(Duration::from_secs(42))
        );
        assert_eq!(
            state
                .upcoming
                .view
                .tracks
                .iter()
                .map(|row| row.audio_source_id)
                .collect::<Vec<_>>(),
            [2, 1]
        );
    }
    assert_eq!(state.tracks.first().unwrap().audio_source_id, 1);
    assert_eq!(state.selected_media.as_ref().unwrap().title, "Alpha");
    updates.lock().unwrap().clear();
    state.replace_tracks(
        vec![track(1, "Zulu", "Alpha"), track(2, "Alpha", "Zulu")],
        false,
    );
    assert_eq!(state.tracks.first().unwrap().title, "Alpha");
    assert_eq!(state.tracks.first().unwrap().artist, "演奏者");
}

#[test]
fn loading_playlist_publishes_retained_caption_and_old_wire_fields_default_to_none() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(vec![track(1, "Song", "曲")], false);
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(10);
    state.playlists.view.loading = true;
    updates.lock().unwrap().clear();
    state.command(AppCommand::SetTrackNamePreferences(TrackNamePreferences {
        use_unicode_titles: true,
        use_unicode_artists: true,
    }));
    assert!(state.playlists.view.active.is_none());
    assert_eq!(state.selected, Some(1));
    assert!(updates.lock().unwrap().iter().any(|update| matches!(update,
        AppUpdate::TrackSelected(Some(row)) if row.title == "曲" && row.artist == "演奏者")));
    let item: crate::models::PlaylistItem = serde_json::from_value(serde_json::json!({
        "id": 1, "playlist_id": 10, "source_kind": "stable", "beatmap_hash": "hash",
        "title": "Song", "artist": "Artist", "difficulty_name": " ",
        "beatmap_id": null, "beatmap_set_id": null, "audio_source_id": null, "cover_beatmap_id": null
    })).unwrap();
    assert_eq!(
        (item.title_unicode.as_ref(), item.artist_unicode.as_ref()),
        (None, None)
    );
    assert_eq!(
        item.track().subtitle,
        "Artist | Unknown difficulty · Unavailable"
    );
}
