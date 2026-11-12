//! Buttons of the bars that frame a pane (navigation, status, Viewer, sidebar),
//! sized like those of the title bar: the full bar height, so the whole cell is the
//! click target, and neighbouring cells touch. Their fill spans the cell, square as
//! the title bar's; [`SQUARE_FILL`] switches to a rounded shape inset in the cell,
//! as Adwaita's window buttons draw a disc in a full-height zone.

use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// Icons at the size of the title bar ones.
const ICON: Rems = rems(1.0);
/// Inner margin of a cell that carries a label or a chevron beside its icon.
const LABELLED_PADDING: Rems = rems(0.5);

/// A full-height icon cell, as wide as `width` (the bar height makes it square).
/// The caller adds its fill, listeners and icon.
pub(crate) fn cell(id: impl Into<ElementId>, width: Rems) -> Stateful<Div> {
    div().id(id).w(width).h_full().flex().items_center().justify_center().flex_none()
}

/// A full-height cell sized to its icon and what follows it.
pub(crate) fn labelled(button: Stateful<Div>, gap: Rems) -> Stateful<Div> {
    button.w_auto().px(LABELLED_PADDING).gap(gap)
}

/// Square fills spanning the whole cell, as the title bar's. `false` draws inset
/// rounded ones instead, with the same click target; kept as an option.
const SQUARE_FILL: bool = true;
/// Space between a cell's edges and its rounded fill.
const FILL_INSET: Rems = rems(0.25);
/// Group of a cell, so its fill follows the pointer over the whole cell.
const GROUP: &str = "bar-button";

/// The fill of a cell: hovered when it acts, selected while its toggle or choice
/// is on, as the Viewer button of the title bar. Call it before adding the cell's
/// content: a rounded fill is its first child, drawn beneath the rest.
pub(crate) fn fill(button: Stateful<Div>, enabled: bool, active: bool, theme: OxydeTheme) -> Stateful<Div> {
    if SQUARE_FILL {
        return button
            .when(active, |button| button.bg(theme.selected))
            .when(enabled && !active, |button| button.hover(move |style| style.bg(theme.control_hover)));
    }
    let shape = div()
        .absolute()
        .top(FILL_INSET)
        .bottom(FILL_INSET)
        .left(FILL_INSET)
        .right(FILL_INSET)
        .rounded_md()
        .when(active, |shape| shape.bg(theme.selected))
        .when(enabled && !active, |shape| shape.group_hover(GROUP, move |style| style.bg(theme.control_hover)));
    button.relative().group(GROUP).child(shape)
}

/// The icon of a cell; `accent` while its toggle or choice is on.
pub(crate) fn icon(path: &'static str, color: Hsla) -> Svg {
    svg().path(path).size(ICON).flex_none().text_color(color)
}

/// The color of a cell's icon.
pub(crate) fn icon_color(enabled: bool, active: bool, theme: OxydeTheme) -> Hsla {
    if !enabled { theme.icon_disabled } else if active { theme.accent } else { theme.text }
}

