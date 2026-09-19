use vizia::prelude::*;

use crate::app::{AppEvent, SettingsRetry, UiState};
use crate::assets;
use crate::views::components::{
    ButtonVariant, MenuItem, button, gap, icon, icon_button, menu, search_row, sidebar,
};

pub(crate) fn settings_pane(cx: &mut Context, state: UiState) -> Handle<'_, VStack> {
    let unavailable = Memo::new(move |_| {
        !state.connected.get()
            || state.settings_loading.get()
            || state.busy.get()
            || state.browsing.get()
    });
    let folders = Memo::new(move |_| {
        state
            .folders
            .get()
            .iter()
            .map(|folder| MenuItem {
                id: folder.id,
                label: format!("{} - {}", folder.kind, folder.root_path),
            })
            .collect::<Vec<_>>()
    });

    sidebar(cx, move |cx| {
        search_row(cx, state.settings_query, "Type to search settings...");
        gap(cx, 40.0);
        ScrollView::new(cx, move |cx| {
            VStack::new(cx, move |cx| {
                HStack::new(cx, |cx| {
                    icon(cx, assets::PENCIL);
                    Label::new(cx, "General");
                })
                .class("section-header");

                VStack::new(cx, move |cx| {
                    Label::new(cx, "osu! folders").class("field-label");
                    HStack::new(cx, move |cx| {
                        menu(
                            cx,
                            folders,
                            state.selected_folder,
                            "No osu! folders",
                            false,
                            move |cx, id| cx.emit(AppEvent::SelectFolder(id)),
                        )
                        .disabled(Memo::new(move |_| {
                            unavailable.get() || state.folders.get().is_empty()
                        }));
                        icon_button(cx, assets::ADD)
                            .name("Add osu! folder")
                            .class("folder-add")
                            .on_press(|cx| cx.emit(AppEvent::Browse))
                            .disabled(unavailable);
                    })
                    .class("folder-picker-row");

                    Label::new(cx, state.settings_message)
                        .class("load-message")
                        .display(state.settings_message.map(|message| !message.is_empty()));
                    button(cx, "Retry", ButtonVariant::Alternate)
                        .class("settings-retry")
                        .display(state.settings_retry.map(Option::is_some))
                        .disabled(Memo::new(move |_| {
                            state.settings_loading.get() || state.busy.get() || state.browsing.get()
                        }))
                        .on_press(move |cx| {
                            cx.emit(match state.settings_retry.get() {
                                Some(SettingsRetry::Browse) => AppEvent::Browse,
                                _ => AppEvent::RefreshSettings,
                            });
                        });
                })
                .class("field");
            })
            .class("settings-section");
        })
        .show_horizontal_scrollbar(false)
        .class("settings-list");
    })
}

pub(crate) fn style() -> CSS {
    include_style!("styles/settings.css")
}
