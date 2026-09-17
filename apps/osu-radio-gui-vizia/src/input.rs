//! Shared input policy for both desktop entry points.
use vizia::prelude::*;

const WHEEL_LOGICAL_PIXELS: f32 = 60.0;
// Vizia 0.4 ScrollView multiplies MouseScroll deltas by 20 against physical bounds.
// Recheck this adapter when upgrading Vizia. Wheel and touchpad deltas are combined
// by its window backend, so this policy necessarily affects both.
const VIZIA_SCROLL_PIXELS: f32 = 20.0;

pub(crate) fn install(cx: &mut Context) {
    // Installed once per application, before views. Global listeners run once before
    // routing, unlike per-view handlers that can scale again as an event bubbles.
    cx.add_global_listener(|cx, event| adjust_scroll(event, cx.scale_factor()));
}

fn adjust_scroll(event: &mut Event, scale_factor: f32) {
    let mut replacement = None;
    event.map(|message, meta| {
        if let WindowEvent::MouseScroll(x, y) = message {
            let factor = WHEEL_LOGICAL_PIXELS * scale_factor / VIZIA_SCROLL_PIXELS;
            replacement = Some(
                Event::new(WindowEvent::MouseScroll(x * factor, y * factor))
                    .target(meta.target)
                    .origin(meta.origin)
                    .propagate(meta.propagation),
            );
        }
    });
    if let Some(replacement) = replacement {
        // Replace in place: do not emit a new event (which would be scaled again).
        // Native routing, Shift-axis handling and ScrollView bounds remain in charge.
        *event = replacement;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    use vizia::events::EventManager;

    #[derive(Clone)]
    struct ScrollProbe(Rc<RefCell<Vec<(Entity, f32, f32)>>>);

    impl View for ScrollProbe {
        fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
            event.map(|message, _| {
                if let WindowEvent::MouseScroll(x, y) = message {
                    self.0.borrow_mut().push((cx.current(), *x, *y));
                }
            });
        }
    }

    #[test]
    fn installed_listener_adjusts_once_before_bubbling_through_nested_views() {
        let mut cx = Context::new();
        install(&mut cx);
        let received = Rc::new(RefCell::new(Vec::new()));
        let probe = ScrollProbe(received.clone());
        let mut child = Entity::root();
        let parent = probe
            .clone()
            .build(&mut cx, |cx| {
                child = probe.build(cx, |_| {}).entity();
            })
            .entity();
        cx.emit_custom(Event::new(WindowEvent::MouseScroll(0.25, -1.0)).target(child));
        EventManager::new().flush_events(&mut cx, |_| {});
        assert_eq!(
            *received.borrow(),
            vec![(child, 0.75, -3.0), (parent, 0.75, -3.0)]
        );
    }

    #[test]
    fn scroll_keeps_direction_fractional_deltas_and_logical_distance_at_each_scale() {
        for scale in [1.0, 1.5, 2.0] {
            for (x, y) in [
                (0.0, 1.0),
                (0.0, -1.0),
                (0.25, -0.5),
                (-1.0, 0.0),
                (0.0, 0.0),
            ] {
                let mut event = Event::new(WindowEvent::MouseScroll(x, y));
                adjust_scroll(&mut event, scale);
                let mut observed = None;
                event.map(|message, _| {
                    if let WindowEvent::MouseScroll(x, y) = message {
                        // ScrollView converts to a fraction of physical overflow, then
                        // divides physical offsets by display scale for logical layout.
                        observed = Some((
                            x * VIZIA_SCROLL_PIXELS / scale,
                            y * VIZIA_SCROLL_PIXELS / scale,
                        ));
                    }
                });
                assert_eq!(observed, Some((x * 60.0, y * 60.0)));
            }
        }
    }

    #[test]
    fn scroll_preserves_target_origin_and_each_propagation_mode() {
        for propagation in [Propagation::Up, Propagation::Direct, Propagation::Subtree] {
            let target = Entity::new(7, 1);
            let origin = Entity::new(12, 2);
            let mut event = Event::new(WindowEvent::MouseScroll(0.0, -1.0))
                .target(target)
                .origin(origin)
                .propagate(propagation);
            adjust_scroll(&mut event, 1.0);
            let mut observed = None;
            event.map(|_: &WindowEvent, meta| {
                observed = Some((meta.target, meta.origin, meta.propagation));
            });
            assert_eq!(observed, Some((target, origin, propagation)));
        }
    }

    #[test]
    fn non_scroll_window_and_application_events_are_untouched() {
        let mut window_event = Event::new(WindowEvent::MouseMove(12.5, -3.0));
        adjust_scroll(&mut window_event, 2.0);
        let mut observed = None;
        window_event.map(|message, _| {
            if let WindowEvent::MouseMove(x, y) = message {
                observed = Some((*x, *y));
            }
        });
        assert_eq!(observed, Some((12.5, -3.0)));

        let mut app_event = Event::new(String::from("unchanged"));
        adjust_scroll(&mut app_event, 2.0);
        let mut observed = None;
        app_event.map(|message: &String, _| observed = Some(message.clone()));
        assert_eq!(observed.as_deref(), Some("unchanged"));
    }
}
