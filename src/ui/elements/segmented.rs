use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// Corner radius of the frame.
const RADIUS: Rems = rems(0.25);
/// Space above and below the frame.
const MARGIN: Rems = rems(0.125);
/// Space between the frame's outline and its options.
const PADDING: Rems = rems(0.125);
/// Height of an option: what remains of 2.25 rem once the margin, the outline
/// (1/16 rem on each side) and the padding are taken.
pub const SEGMENT_HEIGHT: Rems = rems(2.25 - 0.125 * 2.0 - 0.0625 * 2.0 - 0.125 * 2.0);
/// Radius of an option's corners that meet the frame's: the frame's, less the outline
/// and the padding, so both curves stay parallel.
const INNER_RADIUS: Rems = rems(0.25 - 0.0625 - 0.125);

/// Mutually exclusive options drawn as one control, also framing the − value + stepper.
/// The fill and outline derive from the text color, so they contrast on every theme and
/// surface. `ring` replaces the outline, for a group reached by the keyboard.
/// The caller owns the selection, focus routing and actions.
///
/// It stands 2.25 rem high, as the bars: a margin above and below keeps the frame
/// off its parent's edges, the frame holds its padding and the options.
pub fn segmented(id: impl Into<ElementId>, ring: Option<Hsla>, theme: OxydeTheme) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(rems(0.125))
        .my(MARGIN)
        .p(PADDING)
        .rounded(RADIUS)
        .bg(theme.text.opacity(0.05))
        .border(rems(0.0625))
        .border_color(ring.unwrap_or(outline(theme)))
}

/// One option of [`segmented`]. A disabled option must also omit its action.
/// [`SegmentPlace::place`] rounds the corners it shares with the frame.
pub fn segment(id: impl Into<ElementId>, selected: bool, enabled: bool, theme: OxydeTheme) -> Stateful<Div> {
    div()
        .id(id)
        .h(SEGMENT_HEIGHT)
        .px(rems(0.625))
        .flex()
        .items_center()
        .justify_center()
        .gap(rems(0.375))
        .min_w_0()
        .whitespace_nowrap()
        .text_color(if !enabled {
            theme.icon_disabled
        } else if selected {
            theme.accent
        } else {
            theme.text
        })
        .when(selected, |segment| segment.bg(theme.selected).font_weight(FontWeight::MEDIUM))
        .when(enabled && !selected, |segment| {
            segment
                .hover(move |style| style.bg(hover_fill(theme)))
                .active(move |style| style.bg(theme.text.opacity(0.14)))
        })
}

/// Position of an option in its [`segmented`] frame.
pub trait SegmentPlace {
    /// Rounds the corners of option `index` among `count` that meet the frame's corners.
    fn place(self, index: usize, count: usize) -> Self;
}

impl SegmentPlace for Stateful<Div> {
    fn place(self, index: usize, count: usize) -> Self {
        self.when(index == 0, |option| option.rounded_l(INNER_RADIUS))
            .when(index + 1 >= count, |option| option.rounded_r(INNER_RADIUS))
    }
}

/// The resting outline of [`segmented`], for a caller that only colors it while focused.
pub fn outline(theme: OxydeTheme) -> Hsla {
    theme.text.opacity(0.16)
}

/// Hover of an option, translucent over the group's fill so it shows on every theme.
pub fn hover_fill(theme: OxydeTheme) -> Hsla {
    theme.text.opacity(0.08)
}
