use crate::models::AudioModsProfile;
use crate::view_models::{AudioModsState, ModsAction, mods::Editor};

/// Uses the same editing rules as `SettingsViewModel`, without timers, network or audio.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MockAudioMods {
    editor: Editor,
}
impl MockAudioMods {
    pub(crate) fn select(&mut self, id: Option<i32>) {
        self.editor.select(id);
    }
    #[must_use]
    pub fn state(&self) -> AudioModsState {
        let target = self.editor.target();
        let id = target.and_then(|t| t.audio_id);
        AudioModsState {
            settings: self.editor.settings,
            loaded: true,
            loading: false,
            saving: false,
            error: String::new(),
            target,
            selected_profile: self.editor.effective(id, None),
            selected_has_override: id.and_then(|id| self.editor.stored(id, None)).is_some(),
        }
    }
    #[must_use]
    pub fn effective(&self, audio_id: i32) -> AudioModsProfile {
        self.editor.effective(Some(audio_id), None)
    }
    pub fn apply(&mut self, action: ModsAction) -> bool {
        match action {
            ModsAction::SetIndividual(enabled) => self.editor.set_individual(enabled).is_some(),
            ModsAction::Edit { target, edit } => self.editor.edit(target, edit, None).is_some(),
            ModsAction::Finish(target) if self.editor.target() == Some(target) => {
                self.editor.invalidate();
                true
            }
            ModsAction::Finish(_) | ModsAction::Retry => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mock::{Action, MockState},
        view_models::ModsEdit,
    };
    #[test]
    fn offline_state_uses_shared_rules_and_rejects_old_selection_events() {
        let mut state = MockState::default();
        let target = state.mods.state().target.unwrap();
        assert!(state.apply(Action::Mods(ModsAction::Edit {
            target,
            edit: ModsEdit::CycleDt
        })));
        assert_eq!(state.mods.state().selected_profile.speed_percent, 150);
        assert!(state.apply(Action::Mods(ModsAction::SetIndividual(true))));
        assert_eq!(
            state.mods.state().selected_profile,
            AudioModsProfile::default()
        );
        let target = state.mods.state().target.unwrap();
        let id = target.audio_id.unwrap();
        assert_eq!(
            Some(id),
            state
                .tracks
                .get(state.selected)
                .and_then(|track| i32::try_from(track.id).ok())
        );
        assert!(state.apply(Action::Mods(ModsAction::Edit {
            target,
            edit: ModsEdit::ToggleHt
        })));
        assert!(state.apply(Action::SelectTrack(1)));
        assert!(!state.apply(Action::Mods(ModsAction::Edit {
            target,
            edit: ModsEdit::Clear
        })));
        assert_eq!(state.mods.effective(id).speed_percent, 75);
        assert_eq!(
            state.mods.state().selected_profile,
            AudioModsProfile::default()
        );
        state.apply(Action::Playlist(
            crate::view_models::PlaylistAction::Select(Some(1)),
        ));
        state.apply(Action::Playlist(
            crate::view_models::PlaylistAction::SelectItem(1),
        ));
        let target = state.mods.state().target.unwrap();
        assert_eq!(target.audio_id, Some(1));
        state.apply(Action::Mods(ModsAction::Edit {
            target,
            edit: ModsEdit::CycleDt,
        }));
        state.apply(Action::Playlist(
            crate::view_models::PlaylistAction::SelectItem(2),
        ));
        assert_eq!(
            state.mods.state().selected_profile.speed_percent,
            150,
            "playlist difficulties share audio 1"
        );
    }
}
