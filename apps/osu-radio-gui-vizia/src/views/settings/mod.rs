use vizia::prelude::*;

use crate::app::{AppEvent, SettingsRetry, UiState};
use crate::assets;
use crate::views::components::{gap, icon, search_row, sidebar};

pub(crate) fn settings_pane(cx: &mut Context, state: UiState) -> Handle<'_, VStack> {
    let unavailable = Memo::new(move |_| {
        !state.connected.get()
            || state.settings_loading.get()
            || state.busy.get()
            || state.browsing.get()
    });
    let folder_name = Memo::new(move |_| {
        state
            .folders
            .get()
            .iter()
            .find(|folder| Some(folder.id) == state.selected_folder.get())
            .map_or_else(
                || "No osu! folders".to_owned(),
                |folder| format!("{} - {}", folder.kind, folder.root_path),
            )
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
                        Dropdown::new(
                            cx,
                            move |cx| {
                                Button::new(cx, move |cx| {
                                    HStack::new(cx, move |cx| {
                                        Label::new(cx, folder_name).class("folder-name");
                                        icon(cx, assets::CHEVRON);
                                    })
                                    .class("folder-trigger-content")
                                })
                                .name("osu! folders")
                                .class("folder-trigger")
                                .on_press(|cx| cx.emit(PopupEvent::Switch))
                                .disabled(Memo::new(move |_| {
                                    unavailable.get() || state.folders.get().is_empty()
                                }))
                                .tooltip(move |cx| {
                                    Tooltip::new(cx, move |cx| {
                                        Label::new(cx, folder_name).class("folder-tooltip-text");
                                    })
                                });
                            },
                            move |cx| folder_options(cx, state),
                        )
                        .show_arrow(false)
                        .placement(Placement::BottomStart)
                        .class("folder-dropdown");

                        Button::new(cx, |cx| icon(cx, assets::ADD))
                            .name("Add osu! folder")
                            .class("folder-add")
                            .on_press(|cx| cx.emit(AppEvent::Browse))
                            .disabled(unavailable);
                    })
                    .class("folder-picker-row");

                    Label::new(cx, state.settings_message)
                        .class("load-message")
                        .display(state.settings_message.map(|message| !message.is_empty()));
                    Button::new(cx, |cx| Label::new(cx, "Retry"))
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

fn folder_options(cx: &mut Context, state: UiState) {
    ScrollView::new(cx, move |cx| {
        VStack::new(cx, move |cx| {
            Binding::new(cx, state.folders, move |cx| {
                for folder in state.folders.get() {
                    let id = folder.id;
                    let name = format!("{} - {}", folder.kind, folder.root_path);
                    Button::new(cx, move |cx| {
                        Label::new(cx, name).class("folder-option-text")
                    })
                    .class("folder-option")
                    .checked(
                        state
                            .selected_folder
                            .map(move |selected| *selected == Some(id)),
                    )
                    .on_press(move |cx| {
                        state.selected_folder.set(Some(id));
                        cx.emit(PopupEvent::Close);
                    });
                }
            });
        })
        .class("folder-options");
    })
    .show_horizontal_scrollbar(false)
    .class("folder-popup");
}

pub(crate) fn style() -> CSS {
    include_style!("styles/settings.css")
}
