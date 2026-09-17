use vizia::prelude::*;

use crate::app::{AppEvent, Tab, UiState};
use crate::assets;
use crate::views::components::{hspacer, icon, icon_button};

/// The window is undecorated, so this row is the title bar: the empty space in it drags the
/// window and the trailing group replaces the system minimize/maximize/close buttons.
pub(crate) fn top_bar(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            nav_tab(cx, state, Tab::Songs, assets::MUSIC, "Songs");
            nav_tab(cx, state, Tab::Settings, assets::SETTINGS, "Settings");
        })
        .class("tabs");

        hspacer(cx);
        server_status(cx, state);

        icon_button(cx, assets::STACK)
            .name("Playlists (unavailable)")
            .disabled(true);
        window_controls(cx, state);
    })
    .class("top-bar")
    // `MouseDown` bubbles, so a press on a child arrives here too; only a press that landed on
    // the bar itself may start a drag.
    .on_mouse_down(|cx, button| {
        if button == MouseButton::Left && cx.hovered() == cx.current() {
            cx.emit(AppEvent::DragWindow);
        }
    })
    .on_double_click(|cx, button| {
        if button == MouseButton::Left && cx.hovered() == cx.current() {
            cx.emit(AppEvent::ToggleMaximizeWindow);
        }
    });
}

fn nav_tab(cx: &mut Context, state: UiState, tab: Tab, glyph: &'static [u8], text: &'static str) {
    Button::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            icon(cx, glyph);
            Label::new(cx, text).class("nav-label");
        })
        .class("nav-content")
    })
    .class("ui-button")
    .class("nav-tab")
    .name(text)
    .toggle_class("active", state.tab.map(move |current| *current == tab))
    .on_press(move |cx| cx.emit(AppEvent::SelectTab(tab)));
}

fn server_status(cx: &mut Context, state: UiState) {
    Label::new(cx, state.status)
        .class("server-status")
        .display(state.status.map(|status| !status.is_empty()));
}

fn window_controls(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        window_button(
            cx,
            |cx| {
                icon(cx, assets::MINIMIZE);
            },
            |cx| cx.emit(AppEvent::MinimizeWindow),
        )
        .name("Minimize");

        window_button(
            cx,
            move |cx| {
                icon(cx, assets::MAXIMIZE).display(state.maximized.map(|maximized| !*maximized));
                icon(cx, assets::RESTORE).display(state.maximized);
            },
            |cx| cx.emit(AppEvent::ToggleMaximizeWindow),
        )
        .name("Maximize / restore");

        window_button(
            cx,
            |cx| {
                icon(cx, assets::CLOSE);
            },
            |cx| cx.emit(AppEvent::CloseWindow),
        )
        .class("close")
        .name("Close window");
    })
    .class("window-controls");
}

fn window_button(
    cx: &mut Context,
    content: impl Fn(&mut Context) + 'static,
    action: impl Fn(&mut EventContext) + Send + Sync + 'static,
) -> Handle<'_, Button> {
    Button::new(cx, move |cx| {
        HStack::new(cx, content).class("window-button-content")
    })
    .class("ui-button")
    .class("window-button")
    .on_press(action)
}

pub(crate) fn style() -> CSS {
    include_style!("styles/top-bar.css")
}
