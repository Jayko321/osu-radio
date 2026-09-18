//! Qt-only projection of the shared, memory-only client model.
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use osu_radio_client::mock::{Action, MockState};
use serde_json::json;
use std::pin::Pin;

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, state_json, cxx_name = "stateJson", READ, NOTIFY)]
        type MockBridge = super::MockBridgeRust;

        #[qinvokable]
        fn dispatch(self: Pin<&mut MockBridge>, action: &QString, value: &QString);
    }
}

pub struct MockBridgeRust {
    state_json: QString,
    state: MockState,
}

fn snapshot(state: &MockState) -> QString {
    let mut value = json!(state);
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
        Self {
            state_json: snapshot(&state),
            state,
        }
    }
}

fn action_from_qml(action: &str, value: String) -> Option<Action> {
    Some(match action {
        "selectTrack" => Action::SelectTrack(value.parse().ok()?),
        "search" => Action::Search(value),
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
    pub fn dispatch(mut self: Pin<&mut Self>, action: &QString, value: &QString) {
        let Some(action) = action_from_qml(&String::from(action), String::from(value)) else {
            return;
        };
        if self.as_mut().rust_mut().state.apply(action) {
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
            ("selectTrack", "-1"),
            ("selectTab", "abc"),
            ("galleryDisabled", "1"),
        ] {
            assert!(action_from_qml(action, value.to_owned()).is_none());
        }
    }

    #[test]
    fn snapshot_exposes_filtered_menu_original_indices() {
        let mut state = MockState::default();
        let query = state.gallery.menu_items.last().unwrap().clone();
        assert!(state.apply(Action::MenuQuery(query.clone())));
        let value: serde_json::Value =
            serde_json::from_str(&String::from(snapshot(&state))).unwrap();
        assert_eq!(value["gallery"]["menu_results"][0]["label"], query);
        assert_eq!(
            value["gallery"]["menu_results"][0]["index"],
            state.gallery.menu_items.len() - 1
        );
    }
}
