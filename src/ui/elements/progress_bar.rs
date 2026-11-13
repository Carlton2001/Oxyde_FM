//! A determinate progress bar: a rail, a fill that glides between worker updates
//! and, while the work finalizes past its last byte, a reflection sweeping the fill.
use super::{motion, rounded_fill::rounded_fill};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

const HEIGHT: f32 = 0.3125;
const SWEEP: std::time::Duration = std::time::Duration::from_millis(1800);

/// `id` names the spring and the sweep, so it stays the same for one operation.
pub(crate) fn progress_bar(
    id: impl Into<SharedString>,
    fraction: f32,
    color: Hsla,
    finalizing: bool,
    cx: &App,
) -> Div {
    let id: SharedString = id.into();
    let theme = *cx.theme();
    let sweep = finalizing && !cx.reduce_motion();
    div()
        .relative()
        .w_full()
        .h(rems(HEIGHT))
        .flex_none()
        .rounded_full()
        .bg(theme.control_hover)
        .overflow_hidden()
        .child(div().absolute().size_full().with_spring(
            SharedString::from(format!("{id}-fill")),
            motion::slide(fraction.clamp(0.0, 1.0)),
            move |fill, fraction| {
                fill.child(
                    rounded_fill(fraction, rems(HEIGHT / 2.0), color)
                        .absolute()
                        .size_full(),
                )
            },
        ))
        .when(sweep, |bar| {
            let shine = theme.text_on_accent;
            let half = |from: f32, to: f32| {
                div().flex_1().h_full().bg(linear_gradient(
                    90.0,
                    linear_color_stop(shine.opacity(from), 0.0),
                    linear_color_stop(shine.opacity(to), 1.0),
                ))
            };
            bar.child(
                div()
                    .absolute()
                    .left(rems(HEIGHT / 2.0))
                    .right(rems(HEIGHT / 2.0))
                    .h_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .h_full()
                            .w(relative(0.4))
                            .flex()
                            .child(half(0.0, 0.45))
                            .child(half(0.45, 0.0))
                            .with_animation(
                                SharedString::from(format!("{id}-sweep")),
                                Animation::new(SWEEP).repeat().with_easing(ease_in_out),
                                |reflection, delta| reflection.left(relative(-0.4 + 1.4 * delta)),
                            ),
                    ),
            )
        })
}
