//! Qt-only projection of the shared, memory-only component gallery.
use crate::qt_view_models::{parse_entry_id, queue_move};
use crate::runtime::ffi as native;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QVariant};
use osu_radio_client::mock::{Action, MockState};
use osu_radio_client::models::{QueueItemRef, QueuePlacement};
use osu_radio_client::view_models::{PlaylistAction, QueueAction};
use serde_json::json;
use std::pin::Pin;

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }
    unsafe extern "C++" {
        include!("src/qt_view_models.cxxqt.h");
        type TrackListModel = crate::qt_view_models::ffi::TrackListModel;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, state_json, cxx_name = "stateJson", READ, NOTIFY)]
        #[qproperty(*mut TrackListModel, queue_tracks, cxx_name = "queueModel", READ = queue_pointer, CONSTANT)]
        type MockBridge = super::MockBridgeRust;
        fn queue_pointer(self: &MockBridge) -> *mut TrackListModel;

        #[qinvokable]
        fn dispatch(self: Pin<&mut MockBridge>, action: &QString, value: &QString);
    }
}

pub struct MockBridgeRust {
    state_json: QString,
    state: MockState,
    model: cxx::UniquePtr<crate::qt_view_models::ffi::TrackListModel>,
}

fn snapshot(state: &MockState) -> QString {
    let mut value = json!({
        "queue": {
            "entries": state.queue.view.entries.iter().map(|row| json!({
                "id": row.id, "audio_source_id": row.audio_source_id,
                "playlist_item_id": row.playlist_item_id, "availability": row.availability,
                "title": row.track.title, "artist": row.track.artist,
            })).collect::<Vec<_>>(),
            "current_entry_id": state.queue.view.current_entry_id,
            "history_len": state.queue.view.history_len,
            "mode": state.queue.view.mode,
            "waiting_for_check": state.queue.view.waiting_for_check,
            "revision": state.queue.view.revision,
            "playback_token": state.queue.view.playback_token,
            "operation_error": state.queue.view.operation_error,
        },
        "gallery": state.gallery,
        "tracks": state.tracks,
        "selected": state.selected,
        "playlists": state.gallery.playlists.view,
        "filtered_playlists": state.gallery.playlists.view.filtered_playlists(),
        "playlist_cover_present": state.gallery.playlists.view.editor_cover_png.is_some(),
        "track_sort_index": crate::app_bridge::track_sort_index(state.track_sort),
    });
    let results = json!(
        state
            .gallery
            .filtered_menu()
            .into_iter()
            .map(|(index, label)| json!({"index": index, "label": label}))
            .collect::<Vec<_>>()
    );
    if let Some(gallery) = value
        .get_mut("gallery")
        .and_then(serde_json::Value::as_object_mut)
    {
        gallery.insert("menu_results".to_owned(), results);
    }
    QString::from(value.to_string())
}

impl Default for MockBridgeRust {
    fn default() -> Self {
        let state = MockState::default();
        let mut model = native::new_track_list_model();
        model.pin_mut().replace_queue_rows(&state.queue.view, 0);
        Self {
            model,
            state_json: snapshot(&state),
            state,
        }
    }
}

fn queue_insert(value: &str) -> Option<QueueAction> {
    let body: serde_json::Value = serde_json::from_str(value).ok()?;
    let placement = match body.get("placement")?.as_str()? {
        "end" => QueuePlacement::End,
        "next" => QueuePlacement::Next,
        _ => return None,
    };
    let items = body
        .get("items")?
        .as_array()?
        .iter()
        .map(|item| {
            let positive_id = |key| {
                let id = i32::try_from(item.get(key)?.as_i64()?).ok()?;
                (id > 0).then_some(id)
            };
            Some(match item.get("type")?.as_str()? {
                "audio" => QueueItemRef::Audio {
                    audio_source_id: positive_id("audio_source_id")?,
                },
                "playlist_item" => QueueItemRef::PlaylistItem {
                    playlist_id: positive_id("playlist_id")?,
                    item_id: positive_id("item_id")?,
                },
                _ => return None,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if items.is_empty() {
        return None;
    }
    Some(QueueAction::Insert { items, placement })
}

fn action_from_qml(action: &str, value: String) -> Option<Action> {
    Some(match action {
        "queueInsert" => Action::Queue(queue_insert(&value)?),
        "queuePlay" => Action::Queue(QueueAction::Play(parse_entry_id(&QString::from(value))?)),
        "queueRemove" => Action::Queue(QueueAction::Remove(parse_entry_id(&QString::from(value))?)),
        "queueMove" => {
            let body: serde_json::Value = serde_json::from_str(&value).ok()?;
            let id = QString::from(body.get("id")?.as_str()?);
            let before = match body.get("before_entry_id")? {
                serde_json::Value::Null => QVariant::default(),
                value => QVariant::from(&QString::from(value.as_str()?)),
            };
            Action::Queue(queue_move(&id, &before)?)
        }
        "queueClearUpcoming" => Action::Queue(QueueAction::ClearUpcoming),
        "queueClearAll" => Action::Queue(QueueAction::ClearAll),
        "queueRefresh" => Action::Queue(QueueAction::Refresh),
        "trackSort" => Action::SetTrackSort(crate::app_bridge::track_sort_from_index(
            value.parse().ok()?,
        )?),
        "playlistShow" => Action::Playlist(PlaylistAction::ShowPlaylists),
        "playlistLibrary" => Action::Playlist(PlaylistAction::ShowLibrary),
        "playlistSearch" => Action::Playlist(PlaylistAction::Search(value)),
        "playlistBeginCreate" => Action::Playlist(PlaylistAction::BeginCreate),
        "playlistBeginEdit" => Action::Playlist(PlaylistAction::BeginEdit(value.parse().ok()?)),
        "playlistEditorName" => Action::Playlist(PlaylistAction::SetEditorName(value)),
        "playlistEditorCancel" => Action::Playlist(PlaylistAction::CancelEditor),
        "playlistEditorSave" => Action::Playlist(PlaylistAction::SaveEditor),
        "playlistCoverReset" => Action::Playlist(PlaylistAction::ResetEditorCover),
        "playlistCoverChoose" => {
            // The offline gallery demonstrates the same editor state with bundled imagery.
            let epoch = value.parse().ok()?;
            Action::Playlist(PlaylistAction::CompleteCoverPick {
                epoch,
                png: Ok(Some(vec![1])),
            })
        }
        "playlistItemSelect" => Action::Playlist(PlaylistAction::SelectItem(value.parse().ok()?)),
        "playlistPlay" => Action::Playlist(PlaylistAction::Play {
            id: value.parse().ok()?,
            start_item_id: None,
        }),
        "playlistOpen" => Action::Playlist(PlaylistAction::Open),
        "playlistClose" => Action::Playlist(PlaylistAction::Close),
        "playlistAddOpen" => Action::Playlist(PlaylistAction::OpenAdd),
        "playlistCreate" => Action::Playlist(PlaylistAction::Create(value)),
        "playlistRename" => {
            let body: serde_json::Value = serde_json::from_str(&value).ok()?;
            Action::Playlist(PlaylistAction::Rename {
                id: i32::try_from(body.get("id")?.as_i64()?).ok()?,
                name: body.get("name")?.as_str()?.into(),
            })
        }
        "playlistDelete" => Action::Playlist(PlaylistAction::Delete(value.parse().ok()?)),
        "playlistQueue" => Action::Playlist(PlaylistAction::AddToQueue(value.parse().ok()?)),
        "playlistSelect" => {
            let id = value.parse().ok()?;
            Action::Playlist(PlaylistAction::Select((id >= 0).then_some(id)))
        }
        "playlistTarget" => Action::Playlist(PlaylistAction::ChooseTarget(value.parse().ok()?)),
        "playlistDifficulty" => {
            Action::Playlist(PlaylistAction::ToggleDifficulty(value.parse().ok()?))
        }
        "playlistAdd" => Action::Playlist(PlaylistAction::Add),
        "playlistRemoveItem" => Action::Playlist(PlaylistAction::RemoveItem(value.parse().ok()?)),
        "playlistRefresh" => Action::Playlist(PlaylistAction::Refresh),
        "folderReset" => Action::FolderReset,
        "folderToggle" => Action::FolderToggle(value),
        "folderRefresh" => Action::FolderRefresh(value),
        "folderCount" => Action::FolderCount(value),
        "folderBrowse" => Action::FolderBrowse,
        "folderApply" => Action::FolderApply,
        "galleryDisabled" => Action::GalleryDisabled(value.parse().ok()?),
        "press" => Action::Press,
        "field" => Action::Field(value),
        "filled" => Action::Filled(value),
        "gallerySearch" => Action::GallerySearch(value),
        "playlistName" => Action::PlaylistName(value),
        "toggleSwitch" => Action::ToggleSwitch(value.parse().ok()?),
        "selectTab" => Action::SelectTab(value.parse().ok()?),
        "toggleTag" => Action::ToggleTag,
        "cycleFilter" => Action::CycleFilter,
        "menuQuery" => Action::MenuQuery(value),
        "selectMenu" => Action::SelectMenu(value.parse().ok()?),
        _ => return None,
    })
}

impl ffi::MockBridge {
    pub fn queue_pointer(&self) -> *mut crate::qt_view_models::ffi::TrackListModel {
        std::ptr::from_ref(&*self.model).cast_mut()
    }
    pub fn dispatch(mut self: Pin<&mut Self>, action: &QString, value: &QString) {
        let Some(action) = action_from_qml(&String::from(action), String::from(value)) else {
            return;
        };
        if self.as_mut().rust_mut().state.apply(action) {
            let view = self.state.queue.view.clone();
            self.as_mut()
                .rust_mut()
                .model
                .pin_mut()
                .replace_queue_rows(&view, 0);
            let updated = snapshot(&self.rust().state);
            self.as_mut().rust_mut().state_json = updated;
            self.as_mut().state_json_changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_rejects_unknown_and_malformed_actions() {
        for (action, value) in [
            ("missing", ""),
            ("selectTrack", "0"),
            ("search", "unused"),
            ("selectTab", "abc"),
            ("galleryDisabled", "1"),
            ("trackSort", "3"),
            ("trackSort", "invalid"),
        ] {
            assert!(action_from_qml(action, value.to_owned()).is_none());
        }
    }

    #[test]
    fn queue_insert_payload_is_typed_and_validated_before_dispatch() {
        for placement in ["end", "next"] {
            let payload = json!({"placement": placement, "items": [
                {"type": "audio", "audio_source_id": 2},
                {"type": "playlist_item", "playlist_id": 1, "item_id": 3}
            ]});
            let Some(Action::Queue(QueueAction::Insert {
                items,
                placement: actual,
            })) = action_from_qml("queueInsert", payload.to_string())
            else {
                panic!("valid insertion");
            };
            assert_eq!(
                items,
                [
                    QueueItemRef::Audio { audio_source_id: 2 },
                    QueueItemRef::PlaylistItem {
                        playlist_id: 1,
                        item_id: 3
                    }
                ]
            );
            assert_eq!(
                actual,
                if placement == "end" {
                    QueuePlacement::End
                } else {
                    QueuePlacement::Next
                }
            );
        }
        for payload in [
            "{}",
            "[]",
            "bad",
            r#"{"placement":"play","items":[]}"#,
            r#"{"placement":"end","items":[]}"#,
            r#"{"placement":"end","items":[{"type":"audio","audio_source_id":0}]}"#,
            r#"{"placement":"end","items":[{"type":"audio","audio_source_id":"1"}]}"#,
            r#"{"placement":"end","items":[{"type":"audio","audio_source_id":2147483648}]}"#,
            r#"{"placement":"end","items":[{"type":"playlist_item","playlist_id":1}]}"#,
            r#"{"placement":"end","items":[{"type":"playlist_item","playlist_id":-1,"item_id":3}]}"#,
            r#"{"placement":"end","items":[{"type":"unknown","audio_source_id":1}]}"#,
        ] {
            assert!(
                action_from_qml("queueInsert", payload.into()).is_none(),
                "{payload}"
            );
        }
    }

    #[test]
    fn snapshot_exposes_filtered_menu_original_indices() {
        let mut state = MockState::default();
        let query = state.gallery.menu_items.last().unwrap().clone();
        assert!(state.apply(Action::MenuQuery(query.clone())));
        let value: serde_json::Value =
            serde_json::from_str(&String::from(snapshot(&state))).unwrap();
        assert_eq!(value["track_sort_index"], 0);
        assert_eq!(value["gallery"]["menu_results"][0]["label"], query);
        assert_eq!(
            value["gallery"]["menu_results"][0]["index"],
            state.gallery.menu_items.len() - 1
        );
    }
}
