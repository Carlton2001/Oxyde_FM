use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

/// Shared switch appearance; the calling row owns interaction and focus.
pub(crate) fn toggle(value: bool, theme: OxydeTheme) -> Div {
    let light = theme.surface.l > theme.text.l;
    let (track_opacity, outline_opacity, thumb_opacity) = if light {
        (0.12, 0.35, 0.70)
    } else {
        (0.16, 0.45, 0.85)
    };
    let track = if value { theme.accent }
        else { theme.surface.blend(theme.text_muted.opacity(track_opacity)) };
    let outline = if value { theme.accent }
        else { theme.surface.blend(theme.text_muted.opacity(outline_opacity)) };
    let thumb = if value { theme.text_on_accent }
        else { theme.surface.blend(theme.text_muted.opacity(thumb_opacity)) };
    div()
        .w(rems(1.875)).h(rems(1.125))
        .rounded_full()
        .border(rems(0.0625)).border_color(outline)
        .bg(track)
        .flex().items_center().flex_none()
        .px(rems(0.0625))
        .when(value, |d| d.justify_end())
        .child(div().size(rems(0.875)).rounded_full().bg(thumb).flex_none())
}
