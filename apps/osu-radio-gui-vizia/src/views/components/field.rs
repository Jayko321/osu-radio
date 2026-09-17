use super::icon;
use crate::assets;
use vizia::prelude::*;

/// The sibling placeholder avoids Vizia 0.4's empty Textbox accessibility underflow.
pub(crate) fn text_input<'a>(
    cx: &'a mut Context,
    query: Signal<String>,
    placeholder: &'static str,
    searchable: bool,
    on_edit: impl Fn(&mut EventContext, String) + Send + Sync + 'static,
) -> Handle<'a, HStack> {
    HStack::new(cx, move |cx| {
        if searchable {
            icon(cx, assets::SEARCH);
        }
        ZStack::new(cx, move |cx| {
            Label::new(cx, placeholder)
                .class("search-placeholder")
                .display(query.map(String::is_empty));
            let textbox = Textbox::new(cx, query)
                .name(placeholder)
                .on_edit(on_edit)
                .entity();
            // Decorative only: CSS follows the textbox's own empty/focus/caret classes,
            // including its existing blink timer. Never insert a placeholder character.
            cx.with_current(textbox, |cx| {
                Element::new(cx).class("empty-caret").hoverable(false);
            });
        })
        .class("search-field");
    })
    .class("input-box")
}

pub(crate) fn field<'a>(
    cx: &'a mut Context,
    label: Option<&'static str>,
    hint: Option<&'static str>,
    value: Signal<String>,
    placeholder: &'static str,
    on_edit: impl Fn(&mut EventContext, String) + Send + Sync + 'static,
) -> Handle<'a, VStack> {
    VStack::new(cx, move |cx| {
        if let Some(label) = label {
            Label::new(cx, label).class("field-label");
        }
        text_input(cx, value, placeholder, false, on_edit);
        if let Some(hint) = hint {
            Label::new(cx, hint).class("field-hint");
        }
    })
    .class("field")
}
