use crate::ui::elements::motion;
use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

const WIDTH: f32 = 1.875;
const HEIGHT: f32 = 1.0;
const BORDER: f32 = 0.0625;
const INSET: f32 = 0.0625;
/// The track shares the corners of fields and checkboxes.
const RADIUS: f32 = 0.25;
/// The thumb's corners run parallel to the track's.
const THUMB_RADIUS: f32 = 0.125;

/// Shared switch appearance; the calling row owns interaction and focus.
/// `id` keeps the thumb's slide across renders and must be unique within the row.
pub(crate) fn toggle(id: impl Into<ElementId>, value: bool, theme: OxydeTheme) -> impl IntoElement {
    // Off, the thumb stands out from its track by lightness alone: darker on
    // light themes, lighter on dark ones.
    let light = theme.surface.l > theme.text.l;
    let (track_opacity, outline_opacity, thumb_opacity) = if light {
        (0.14, 0.45, 0.85)
    } else {
        (0.20, 0.55, 1.0)
    };
    let off_track = theme.surface.blend(theme.text_muted.opacity(track_opacity));
    let off_outline = theme.surface.blend(theme.text_muted.opacity(outline_opacity));
    let off_thumb = if light {
        theme.surface.blend(theme.text_muted.opacity(thumb_opacity))
    } else {
        theme.surface.blend(theme.text.opacity(0.85 * thumb_opacity))
    };
    let on = if value { 1.0 } else { 0.0 };
    div()
        .w(rems(WIDTH))
        .h(rems(HEIGHT))
        .flex_none()
        .flex()
        .rounded(rems(RADIUS))
        .border(rems(BORDER))
        .p(rems(INSET))
        .with_spring(id, motion::toward(on, value), move |track, progress| {
            let progress = progress.clamp(0.0, 1.0);
            track
                .bg(mix(off_track, theme.accent, progress))
                .border_color(mix(off_outline, theme.accent, progress))
                // The thumb takes the snapped height left inside the track, and
                // two spacers share the rest: whatever the display scale, its
                // margins stay even and it sits flush at either end.
                .child(spacer(progress))
                .child(
                    div()
                        .h_full()
                        .aspect_square()
                        .flex_none()
                        .rounded(rems(THUMB_RADIUS))
                        .bg(mix(off_thumb, theme.text_on_accent, progress)),
                )
                .child(spacer(1.0 - progress))
        })
}

/// Takes `share` of the room the thumb leaves free.
fn spacer(share: f32) -> Div {
    let mut spacer = div().flex_basis(px(0.0));
    spacer.style().flex_grow = Some(share);
    spacer.style().flex_shrink = Some(1.0);
    spacer
}

/// Color `progress` of the way from `from` to `to`.
fn mix(from: Hsla, to: Hsla, progress: f32) -> Hsla {
    if progress <= 0.0 {
        return from;
    }
    if progress >= 1.0 {
        return to;
    }
    let from = from.to_rgb();
    let to = to.to_rgb();
    let lerp = |a: f32, b: f32| a + (b - a) * progress;
    Rgba { r: lerp(from.r, to.r), g: lerp(from.g, to.g), b: lerp(from.b, to.b), a: lerp(from.a, to.a) }.into()
}
