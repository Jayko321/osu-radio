use super::text_input;
use vizia::prelude::*;

pub(crate) fn search_row(cx: &mut Context, query: Signal<String>, placeholder: &'static str) {
    text_input(cx, query, placeholder, true, move |_, text| query.set(text)).class("search-row");
}
