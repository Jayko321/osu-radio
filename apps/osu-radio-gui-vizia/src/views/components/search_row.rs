use vizia::prelude::*;

use super::icon;
use crate::assets;

/// The placeholder is drawn as a label rather than through `Textbox::placeholder`, which panics
/// with a subtract overflow in vizia 0.4.0's accessibility pass whenever the box is empty.
pub(crate) fn search_row(cx: &mut Context, query: Signal<String>, placeholder: &'static str) {
    HStack::new(cx, move |cx| {
        ZStack::new(cx, move |cx| {
            Label::new(cx, placeholder)
                .class("search-placeholder")
                .display(query.map(String::is_empty));

            Textbox::new(cx, query).on_edit(move |_, text| query.set(text));
        })
        .class("search-field");

        icon(cx, assets::SEARCH);
    })
    .class("search-row");
}
