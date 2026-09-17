use super::{MaterialKind, icon_button, material};
use crate::assets;
use vizia::prelude::*;

struct ModalOverlay {
    close: std::sync::Arc<dyn Fn(&mut EventContext) + Send + Sync>,
}
impl View for ModalOverlay {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|event, meta| match event {
            WindowEvent::KeyDown(Code::Escape, _) => {
                (self.close)(cx);
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Left) if cx.hovered() == cx.current() => {
                (self.close)(cx);
                meta.consume();
            }
            _ => {}
        });
    }
}

/// Mount at the window overlay root, after the page content. Removing the focus-locked subtree
/// restores Vizia's saved focus to the initiating control.
pub(crate) fn modal(
    cx: &mut Context,
    open: Signal<bool>,
    title: &'static str,
    on_close: impl Fn(&mut EventContext) + Send + Sync + 'static,
    content: impl Fn(&mut Context) + 'static,
) {
    let close = std::sync::Arc::new(on_close);
    let content = std::rc::Rc::new(content);
    Binding::new(cx, open, move |cx| {
        if !open.get() {
            return;
        }
        let close = close.clone();
        let content = content.clone();
        let height = Signal::new(480.0_f32);
        ModalOverlay {
            close: close.clone(),
        }
        .build(cx, |cx| {
            material(cx, MaterialKind::Regular, |cx| {
                ScrollView::new(cx, move |cx| {
                    VStack::new(cx, |cx| {
                        HStack::new(cx, |cx| {
                            Label::new(cx, title).class("modal-title");
                            icon_button(cx, assets::CLOSE)
                                .name("Close dialog")
                                .on_press(move |cx| close(cx));
                        })
                        .class("modal-header");
                        content(cx);
                    })
                    .class("modal-content");
                })
                .show_horizontal_scrollbar(false)
                .height(Auto)
                .max_height(height.map(|value| Pixels(*value)));
            })
            .class("modal-panel")
            .role(Role::Dialog)
            .name(title);
        })
        .class("modal-overlay")
        .on_geo_changed(move |cx, _| {
            // Keep 48px outer clearance plus the panel's two 24px padding edges.
            height.set_if_changed((cx.bounds().h / cx.scale_factor() - 96.0).max(80.0));
        })
        .lock_focus_to_within();
    });
}
