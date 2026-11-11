//! The accent that fades over a resize separator's line while it is hovered or dragged.

use crate::ui::elements::motion;
use gpui::prelude::*;
use gpui::*;

/// Covers its relatively positioned line, fading in when `visible` and out otherwise.
pub(crate) fn resize_highlight(id: impl Into<ElementId>, visible: bool, color: Hsla) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .bg(color)
        .with_spring(id, motion::presence(visible).from(0.0), |line, progress| line.opacity(progress))
}
