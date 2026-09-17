use super::{icon, search_row};
use crate::assets;
use vizia::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MenuItem {
    pub id: i32,
    pub label: String,
}

pub(crate) fn matching_items(items: &[MenuItem], query: &str) -> Vec<MenuItem> {
    let query = query.trim().to_lowercase();
    items
        .iter()
        .filter(|item| item.label.to_lowercase().contains(&query))
        .cloned()
        .collect()
}

/// Native buttons provide Enter/Space; arrows and Home/End move focus and scroll it into view.
struct MenuKeyboard {
    targets: Signal<Vec<Entity>>,
}
impl View for MenuKeyboard {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|event, meta| {
            let WindowEvent::KeyDown(code, _) = event else {
                return;
            };
            let targets = self.targets.get();
            let current = targets.iter().position(|entity| *entity == cx.focused());
            let Some(index) = menu_index(current, targets.len(), *code) else {
                return;
            };
            if let Some(target) = targets.get(index) {
                cx.with_current(*target, |cx| {
                    cx.focus_with_visibility(true);
                    cx.emit(ScrollEvent::ScrollToView(*target));
                });
                meta.consume();
            }
        });
    }
}

fn menu_index(current: Option<usize>, count: usize, key: Code) -> Option<usize> {
    let last = count.checked_sub(1)?;
    match key {
        Code::ArrowDown => {
            Some(current.map_or(0, |i| if i >= last { 0 } else { i.saturating_add(1) }))
        }
        Code::ArrowUp => Some(current.map_or(last, |i| i.checked_sub(1).unwrap_or(last))),
        Code::Home if current.is_some() => Some(0),
        Code::End if current.is_some() => Some(last),
        _ => None,
    }
}

pub(crate) fn menu<'a>(
    cx: &'a mut Context,
    items: impl Res<Vec<MenuItem>> + 'static,
    selected: Signal<Option<i32>>,
    empty_label: &'static str,
    searchable: bool,
    on_select: impl Fn(&mut EventContext, i32) + Send + Sync + 'static,
) -> Handle<'a, Dropdown> {
    let items = items.to_signal(cx);
    let label = Memo::new(move |_| {
        items
            .get()
            .iter()
            .find(|item| Some(item.id) == selected.get())
            .map_or_else(|| empty_label.to_owned(), |item| item.label.clone())
    });
    let action = std::sync::Arc::new(on_select);
    Dropdown::new(
        cx,
        move |cx| {
            Button::new(cx, move |cx| {
                HStack::new(cx, move |cx| {
                    Label::new(cx, label).class("menu-label");
                    icon(cx, assets::CHEVRON);
                })
                .class("menu-trigger-content")
            })
            .class("ui-button")
            .class("menu-trigger")
            .name(empty_label)
            .on_press(|cx| cx.emit(PopupEvent::Switch))
            .tooltip(move |cx| {
                Tooltip::new(cx, move |cx| {
                    Label::new(cx, label).class("menu-tooltip");
                })
            });
        },
        move |cx| {
            let query = Signal::new(String::new());
            let targets = Signal::new(Vec::new());
            let filtered = Memo::new(move |_| matching_items(&items.get(), &query.get()));
            let action = action.clone();
            MenuKeyboard { targets }
                .build(cx, move |cx| {
                    if searchable {
                        search_row(cx, query, "Search options...");
                    }
                    ScrollView::new(cx, move |cx| {
                        Binding::new(cx, filtered, move |cx| {
                            let mut entities = Vec::new();
                            VStack::new(cx, |cx| {
                                let options = filtered.get();
                                if options.is_empty() {
                                    Label::new(cx, "No results").class("field-hint");
                                }
                                for item in options {
                                    let id = item.id;
                                    let action = action.clone();
                                    let entity = Button::new(cx, move |cx| {
                                        Label::new(cx, item.label).class("menu-option-text")
                                    })
                                    .class("ui-button")
                                    .class("menu-option")
                                    .checked(selected.map(move |selected| *selected == Some(id)))
                                    .on_press(move |cx| {
                                        action(cx, id);
                                        cx.emit(PopupEvent::Close);
                                    })
                                    .entity();
                                    entities.push(entity);
                                }
                            })
                            .class("menu-options");
                            targets.set(entities);
                        });
                    })
                    .show_horizontal_scrollbar(false)
                    .class("menu-scroll");
                })
                .class("menu-content")
                .lock_focus_to_within();
        },
    )
    .show_arrow(false)
    .placement(Placement::BottomStart)
    .class("ui-menu")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_search_handles_case_whitespace_unicode_and_no_matches() {
        let items = vec![
            MenuItem {
                id: 5,
                label: "Evening JAZZ".into(),
            },
            MenuItem {
                id: 9,
                label: "Музыка".into(),
            },
        ];
        assert_eq!(
            matching_items(&items, "  jazz "),
            vec![MenuItem {
                id: 5,
                label: "Evening JAZZ".into()
            }]
        );
        assert_eq!(
            matching_items(&items, "МУЗ"),
            vec![MenuItem {
                id: 9,
                label: "Музыка".into()
            }]
        );
        assert_eq!(matching_items(&items, ""), items);
        assert!(matching_items(&items, "missing").is_empty());
    }
    #[test]
    fn keyboard_navigation_wraps_and_handles_empty_results() {
        assert_eq!(menu_index(None, 0, Code::ArrowDown), None);
        assert_eq!(menu_index(None, 3, Code::ArrowDown), Some(0));
        assert_eq!(menu_index(Some(2), 3, Code::ArrowDown), Some(0));
        assert_eq!(menu_index(Some(0), 3, Code::ArrowUp), Some(2));
        assert_eq!(menu_index(Some(1), 3, Code::End), Some(2));
        assert_eq!(menu_index(Some(1), 3, Code::Home), Some(0));
    }
}
