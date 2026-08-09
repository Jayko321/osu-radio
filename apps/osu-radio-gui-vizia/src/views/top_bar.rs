use vizia::prelude::*;

use crate::app::{AppEvent, Tab, UiState};
use crate::assets;
use crate::views::components::{hspacer, icon, icon_button};

pub(crate) fn top_bar(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        nav_tab(cx, state, Tab::Songs, assets::MUSIC, "Songs");
        nav_tab(cx, state, Tab::Settings, assets::SETTINGS, "Settings");

        hspacer(cx);
        server_status(cx, state);

        icon_button(cx, assets::STACK);
    })
    .class("top-bar");
}

fn nav_tab(cx: &mut Context, state: UiState, tab: Tab, glyph: &'static [u8], text: &'static str) {
    HStack::new(cx, move |cx| {
        icon(cx, glyph);
        Label::new(cx, text).class("nav-label");
    })
    .class("nav-tab")
    .toggle_class("active", state.tab.map(move |current| *current == tab))
    .on_press(move |cx| cx.emit(AppEvent::SelectTab(tab)));
}

fn server_status(cx: &mut Context, state: UiState) {
    Label::new(cx, state.status)
        .class("server-status")
        .display(state.status.map(|status| !status.is_empty()));
}

pub(crate) fn style() -> CSS {
    include_style!("styles/top-bar.css")
}
