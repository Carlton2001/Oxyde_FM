//! Fade only clipped path segments, using the geometry of the frame being painted.
use gpui::{prelude::*, *};

pub(super) fn render(
    scroll: ScrollHandle,
    last_segment: Option<usize>,
    surface: Hsla,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            // The scrollable sibling has completed prepaint: offsets are clamped,
            // and both viewport and segment bounds belong to this layout.
            let fades = last_segment
                .and_then(|index| scroll.bounds_for_item(index))
                .map(|last| {
                    edge_fades(
                        scroll.bounds(),
                        last,
                        scroll.offset().x,
                        scroll.max_offset().x,
                        window.rem_size() * (2.0 / 16.0),
                    )
                })
                .unwrap_or((false, false));
            (bounds, fades)
        },
        move |_, (bounds, (left, right)): (Bounds<Pixels>, (bool, bool)), window, _| {
            let width = (window.rem_size() * 1.5).min(bounds.size.width);
            for (visible, x, angle) in [
                (left, bounds.left(), 90.0),
                (right, bounds.right() - width, 270.0),
            ] {
                if visible {
                    window.paint_quad(fill(
                        Bounds::new(point(x, bounds.top()), size(width, bounds.size.height)),
                        linear_gradient(
                            angle,
                            linear_color_stop(surface, 0.0),
                            linear_color_stop(surface.alpha(0.0), 1.0),
                        ),
                    ));
                }
            }
        },
    )
    .absolute()
    .size_full()
}

fn edge_fades(
    viewport: Bounds<Pixels>,
    last_segment: Bounds<Pixels>,
    offset: Pixels,
    max_offset: Pixels,
    tolerance: Pixels,
) -> (bool, bool) {
    let offset = offset.clamp(-max_offset, px(0.0));
    (
        -offset > tolerance,
        // The editable blank area after the final segment is not hidden path.
        last_segment.right() + offset > viewport.right() + tolerance,
    )
}

#[cfg(test)]
#[path = "tests/breadcrumb_fades.rs"]
mod tests;
