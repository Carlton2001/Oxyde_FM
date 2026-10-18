use gpui::{prelude::*, *};

/// Fill a fraction of a rounded surface without squaring its outer corners.
/// The mask crops a full-size rounded quad, preserving exact progress widths
/// even when the filled portion is narrower than the corner diameter.
pub(crate) fn rounded_fill(fraction: f32, radius: Rems, color: Hsla) -> Div {
    div().child(
        canvas(
            |bounds, _, _| bounds,
            move |_, bounds: Bounds<Pixels>, window, _| {
                let fraction = fraction.clamp(0.0, 1.0);
                if fraction <= 0.0 || bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0)
                {
                    return;
                }
                let radius = (window.rem_size() * radius.0)
                    .min(bounds.size.width / 2.0)
                    .min(bounds.size.height / 2.0);
                let mask = ContentMask {
                    bounds: Bounds::new(
                        bounds.origin,
                        size(bounds.size.width * fraction, bounds.size.height),
                    ),
                };
                window.with_content_mask(Some(mask), |window| {
                    window.paint_quad(quad(
                        bounds,
                        radius,
                        color,
                        px(0.0),
                        transparent_black(),
                        BorderStyle::default(),
                    ));
                });
            },
        )
        .size_full(),
    )
}
