//! A horizontal line between groups, as tall as a menu separator so every list,
//! form and card spaces its groups alike.

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
