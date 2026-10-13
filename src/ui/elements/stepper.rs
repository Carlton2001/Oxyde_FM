use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

/// Shared presentation; the caller owns the value, bounds and persistence.
pub fn stepper(
    id: &'static str,
    value: impl IntoElement,
    can_decrease: bool,
    can_increase: bool,
    theme: OxydeTheme,
    on_step: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_step = Rc::new(on_step);
    let button = |increase: bool, enabled: bool| {
        let on_step = on_step.clone();
        let key_step = on_step.clone();
        let row = div()
            .id(SharedString::from(format!(
                "{id}-{}",
                if increase { "increase" } else { "decrease" }
            )))
            .size(rems(1.5))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .text_color(theme.text_muted)
            .when(!enabled, |button| button.opacity(0.4))
            .when(enabled, |button| {
                button.hover(|style| style.bg(theme.control_hover))
            })
            .child(if increase { "+" } else { "−" })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                if enabled {
                    on_step(increase, window, cx);
                }
            });
        if enabled { crate::ui::components::popup::keyboard::item(row, Rc::new(move |window, cx| key_step(increase, window, cx)), None, None).into_any_element() } else { row.into_any_element() }
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .rounded_md()
        .p(rems(0.125))
        .bg(theme.stepper_surface)
        .child(button(false, can_decrease))
        .child(
            div()
                .min_w(rems(3.25))
                .h(rems(1.5))
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.accent)
                .font_features(crate::ui::style::TNUM.clone())
                .child(value),
        )
        .child(button(true, can_increase))
}
