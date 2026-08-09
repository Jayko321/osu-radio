use vizia::prelude::*;

use crate::assets;
use crate::sample::Field;
use crate::views::components::icon;

pub(crate) fn field(cx: &mut Context, field: &'static Field) {
    VStack::new(cx, move |cx| {
        Label::new(cx, field.label).class("field-label");
        field_box(cx, field);
    })
    .class("field");
}

fn field_box(cx: &mut Context, field: &'static Field) {
    HStack::new(cx, move |cx| {
        Label::new(cx, field.value);

        if field.pickable {
            icon(cx, assets::CHEVRON);
        }
    })
    .class("field-box")
    .toggle_class("pickable", field.pickable);
}
