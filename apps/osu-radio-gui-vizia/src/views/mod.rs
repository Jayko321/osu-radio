pub(crate) mod background;
pub(crate) mod components;
pub(crate) mod player;
pub(crate) mod settings;
pub(crate) mod songs;
pub(crate) mod top_bar;

use vizia::prelude::*;

use background::backdrop;
use player::player;
use settings::settings_pane;
use songs::songs_pane;
use top_bar::top_bar;

use crate::app::{Tab, UiState};

/// Every component owns its own sheet. `base` loads first because same-specificity rules resolve
/// last-wins, and the rest are independent of each other.
pub(crate) fn styles(cx: &mut Context) {
    let sheets = [
        include_style!("styles/base.css"),
        background::style(),
        top_bar::style(),
        components::style(),
        songs::style(),
        settings::style(),
        player::style(),
    ];

    for sheet in sheets {
        if let Err(error) = cx.add_stylesheet(sheet) {
            eprintln!("a bundled stylesheet could not be loaded: {error:?}");
        }
    }
}

pub(crate) fn shell(cx: &mut Context, state: UiState) {
    VStack::new(cx, move |cx| {
        top_bar(cx, state);

        ZStack::new(cx, move |cx| {
            backdrop(cx, state);

            HStack::new(cx, move |cx| {
                songs_pane(cx, state).display(state.tab.map(|tab| *tab == Tab::Songs));
                settings_pane(cx, state).display(state.tab.map(|tab| *tab == Tab::Settings));
                player(cx, state);
            })
            .class("panes");
        })
        .class("body");
    })
    .class("app");
}
