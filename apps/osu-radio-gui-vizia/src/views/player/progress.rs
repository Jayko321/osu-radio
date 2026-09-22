use osu_radio_client::{Track, playback::Playback};
use std::time::Duration;
use vizia::prelude::*;

use crate::app::{AppEvent, UiState};
use crate::views::components::hspacer;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Progress {
    audio_id: Option<i32>,
    duration: Option<Duration>,
    position: Duration,
}

fn selected_progress(track: Option<&Track>, playback: &Playback) -> Progress {
    let Some(track) = track else {
        return Progress::default();
    };
    let current = playback.current_audio_id == Some(track.audio_source_id);
    Progress {
        audio_id: current.then_some(track.audio_source_id),
        duration: if current {
            playback.snapshot.duration.or(track.duration)
        } else {
            track.duration
        },
        position: if current {
            playback.snapshot.position
        } else {
            Duration::ZERO
        },
    }
}

fn progress(state: UiState) -> Progress {
    selected_progress(state.selected.get().as_ref(), &state.playback.get())
}

fn seek_target(progress: Progress, fraction: f32) -> Option<(i32, Duration)> {
    let id = progress.audio_id?;
    let duration = progress.duration.filter(|duration| !duration.is_zero())?;
    if !fraction.is_finite() {
        return None;
    }
    Some((id, duration.mul_f32(fraction.clamp(0.0, 1.0))))
}

pub(crate) fn progress_bar(cx: &mut Context, state: UiState) {
    // Draft includes its track ID, so a selection change cannot seek another track.
    let draft = Signal::new(None::<(i32, f32)>);
    seek_slider(cx, state, draft);
}

fn seek_slider(cx: &mut Context, state: UiState, draft: Signal<Option<(i32, f32)>>) -> Entity {
    let value = Memo::new(move |_| {
        let progress = progress(state);
        if let Some((id, value)) = draft.get()
            && progress.audio_id == Some(id)
        {
            return value;
        }
        progress
            .duration
            .filter(|duration| !duration.is_zero())
            .map_or(0.0, |duration| {
                (progress.position.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
            })
    });
    let disabled = Memo::new(move |_| seek_target(progress(state), 0.0).is_none());
    Slider::new(cx, value)
        .class("audio-slider")
        .class("progress-row")
        .name("Playback position")
        .step(0.001_f32)
        .disabled(disabled)
        .on_change(move |cx, fraction| {
            if let Some((id, target)) = seek_target(progress(state), fraction) {
                if cx.mouse().left.state == MouseButtonState::Pressed {
                    draft.set(Some((id, fraction)));
                } else {
                    // Keyboard/accessibility changes are discrete, with no drag to finish.
                    draft.set(None);
                    cx.emit(AppEvent::Seek(id, target));
                }
            }
        })
        .on_mouse_up(move |cx, button| {
            if button != MouseButton::Left {
                return;
            }
            if let Some((id, fraction)) = draft.get() {
                let selected = progress(state);
                if selected.audio_id == Some(id)
                    && let Some((id, target)) = seek_target(selected, fraction)
                {
                    cx.emit(AppEvent::Seek(id, target));
                }
            }
            draft.set(None);
        })
        .entity()
}

fn time_label(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub(crate) fn time_row(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        Label::new(cx, Memo::new(move |_| time_label(progress(state).position)));
        hspacer(cx);
        Label::new(
            cx,
            Memo::new(move |_| {
                progress(state)
                    .duration
                    .map_or_else(|| "--:--".to_owned(), time_label)
            }),
        );
    })
    .class("time-row");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_and_seeking_apply_only_to_selected_current_track() {
        let mut track = Track::new("A", "artist", Duration::from_secs(90));
        track.audio_source_id = 1;
        let mut playback = Playback {
            current_audio_id: Some(1),
            ..Playback::default()
        };
        playback.snapshot.position = Duration::from_secs(30);
        playback.snapshot.duration = Some(Duration::from_secs(100));
        let current = selected_progress(Some(&track), &playback);
        assert_eq!(current.position, Duration::from_secs(30));
        assert_eq!(
            seek_target(current, 0.5),
            Some((1, Duration::from_secs(50)))
        );
        assert_eq!(
            seek_target(current, 1.5),
            Some((1, Duration::from_secs(100)))
        );
        assert!(seek_target(current, f32::NAN).is_none());
        track.audio_source_id = 2;
        let other = selected_progress(Some(&track), &playback);
        assert_eq!(other.position, Duration::ZERO);
        assert_eq!(other.duration, track.duration);
        assert!(seek_target(other, 0.5).is_none());
        assert_eq!(selected_progress(None, &playback), Progress::default());
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    use vizia::events::EventManager;

    struct SeekProbe(Rc<RefCell<Vec<(i32, Duration)>>>);
    impl Model for SeekProbe {
        fn event(&mut self, _: &mut EventContext, event: &mut Event) {
            event.map(|message, _| {
                if let AppEvent::Seek(id, target) = message {
                    self.0.borrow_mut().push((*id, *target));
                }
            });
        }
    }

    #[test]
    fn slider_release_commits_once_and_rejects_selection_changed_during_drag() {
        let mut cx = Context::new();
        let state = UiState::new();
        let mut track = Track::new("A", "Artist", Duration::from_secs(100));
        track.audio_source_id = 1;
        state.selected.set(Some(track.clone()));
        let mut playback = Playback {
            current_audio_id: Some(1),
            ..Playback::default()
        };
        playback.snapshot.duration = Some(Duration::from_secs(100));
        state.playback.set(playback.clone());
        let draft = Signal::new(None);
        let recorded = Rc::new(RefCell::new(Vec::new()));
        SeekProbe(recorded.clone()).build(&mut cx);
        let slider = seek_slider(&mut cx, state, draft);
        let mut events = EventManager::new();
        events.flush_events(&mut cx, |_| {});
        draft.set(Some((1, 0.5)));
        playback.snapshot.position = Duration::from_secs(40);
        state.playback.set(playback);
        events.flush_events(&mut cx, |_| {});
        assert!(recorded.borrow().is_empty());
        assert_eq!(draft.get(), Some((1, 0.5)));
        cx.emit_custom(Event::new(WindowEvent::MouseUp(MouseButton::Left)).target(slider));
        events.flush_events(&mut cx, |_| {});
        assert_eq!(*recorded.borrow(), vec![(1, Duration::from_secs(50))]);
        assert!(draft.get().is_none());
        draft.set(Some((1, 0.8)));
        track.audio_source_id = 2;
        state.selected.set(Some(track));
        cx.emit_custom(Event::new(WindowEvent::MouseUp(MouseButton::Left)).target(slider));
        events.flush_events(&mut cx, |_| {});
        assert_eq!(recorded.borrow().len(), 1);
        assert!(draft.get().is_none());
    }
}
