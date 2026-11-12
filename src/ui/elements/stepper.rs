use crate::ui::theme::OxydeTheme;
use crate::ui::style::TabularFigures;
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

/// Shared presentation, in the frame of [`super::segmented`]; the caller owns the value, bounds and persistence.
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
        let row = super::segmented::segment(
                SharedString::from(format!("{id}-{}", if increase { "increase" } else { "decrease" })),
                false,
                enabled,
                theme,
            )
            .w(rems(1.5))
            .px_0()
            .child(if increase { "+" } else { "−" })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                if enabled {
                    on_step(increase, window, cx);
                }
            });
        if enabled { crate::ui::components::popup::keyboard::item(row, Rc::new(move |window, cx| key_step(increase, window, cx)), None, None).into_any_element() } else { row.into_any_element() }
    };
    super::segmented::segmented(SharedString::from(format!("{id}-stepper")), None, theme)
        .child(button(false, can_decrease))
        .child(
            div()
                .min_w(rems(3.25))
                .h(super::segmented::SEGMENT_HEIGHT)
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.accent)
                .tabular_figures()
                .child(value),
        )
        .child(button(true, can_increase))
}
