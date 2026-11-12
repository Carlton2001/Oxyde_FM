//! Keyboard focus ring drawn around a checkbox, radio or switch.

use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

/// Gap between the control's edge and the ring.
const OFFSET: f32 = 0.125;
const WIDTH: f32 = 0.125;

/// Whether `handle` holds focus that the keyboard brought, the only focus the ring shows.
pub(crate) fn keyboard_focused(handle: &FocusHandle, window: &Window) -> bool {
    handle.is_focused(window) && window.last_input_was_keyboard()
}

/// Surrounds `control` with an accent ring when `visible`, without moving it:
/// the ring overflows the control's box. `round` follows a circular or pill control,
/// otherwise the ring follows the checkbox's corners.
pub(crate) fn focus_ring(control: impl IntoElement, visible: bool, round: bool, theme: OxydeTheme) -> Div {
    let reach = rems(-(OFFSET + WIDTH));
    div().relative().flex_none().flex().child(control).when(visible, |wrapper| {
        wrapper.child(
            div()
                .absolute()
                .top(reach)
                .left(reach)
                .right(reach)
                .bottom(reach)
                .border(rems(WIDTH))
                .border_color(theme.accent)
                .when(round, |ring| ring.rounded_full())
                .when(!round, |ring| ring.rounded(rems(0.25 + OFFSET + WIDTH))),
        )
    })
}
