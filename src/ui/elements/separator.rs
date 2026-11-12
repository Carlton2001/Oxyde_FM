//! A horizontal line between groups, as tall as a menu separator so every list,
//! form and card spaces its groups alike, and the vertical line between groups of a bar.

use gpui::prelude::*;
use gpui::*;

/// A 1/16 rem line of `color`, centered in `style::MENU_SEPARATOR_HEIGHT`.
pub(crate) fn horizontal(color: Hsla) -> Div {
    div()
        .w_full()
        .h(crate::ui::style::MENU_SEPARATOR_HEIGHT)
        .flex()
        .items_center()
        .flex_none()
        .child(div().w_full().h(rems(0.0625)).bg(color))
}

/// A 1 rem high, 1/16 rem wide line of `color` between two groups of a bar, with 0.5 rem on
/// each side so it never touches either group.
pub(crate) fn vertical(color: Hsla) -> Div {
    div().flex_none().w(rems(0.0625)).h(rems(1.0)).mx(rems(0.5)).bg(color)
}
