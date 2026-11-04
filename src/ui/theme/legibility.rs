//! Minimum separations between structural tokens and the surfaces they sit on.
//! Imported palettes keep their hue; only tokens too close to their surfaces move toward the text.

use super::OxydeTheme;
use gpui::{Hsla, Rgba};

/// Pane, sidebar and header separators.
pub(super) const BORDER: f32 = 1.18;
/// Menu and popover outlines, drawn over arbitrary content.
pub(super) const MENU_BORDER: f32 = 1.3;
/// Secondary button fill and ghost hover against the surfaces holding them.
pub(super) const CONTROL: f32 = 1.2;
/// Row hover against both pane backgrounds.
pub(super) const ROW_HOVER: f32 = 1.15;

pub(super) fn luminance(color: Hsla) -> f32 {
    let color: Rgba = color.into();
    let linear = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

pub(super) fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn worst(color: Hsla, surfaces: &[Hsla]) -> f32 {
    surfaces.iter().map(|s| contrast(color, *s)).fold(f32::INFINITY, f32::min)
}

/// Smallest blend of `color` toward `toward` reaching `min` against every surface.
fn reach(color: Hsla, toward: Hsla, surfaces: &[Hsla], min: f32) -> Hsla {
    if worst(color, surfaces) >= min {
        return color;
    }
    (1..=50)
        .map(|step| color.blend(toward.opacity(step as f32 / 50.0)))
        .find(|candidate| worst(*candidate, surfaces) >= min)
        .unwrap_or(toward)
}

pub(super) fn enforce(theme: &mut OxydeTheme) {
    let t = *theme;
    let panes = [t.background, t.active_background, t.sidebar, t.header, t.surface];
    theme.border = reach(t.border, t.text, &panes, BORDER);
    theme.menu_border = reach(t.menu_border, t.text, &[t.menu_surface], MENU_BORDER);
    let holders = [t.surface, t.active_background, t.header, t.menu_surface];
    theme.control_hover = reach(t.control_hover, t.text, &holders, CONTROL);
    theme.hover = reach(t.hover, t.text, &[t.background, t.active_background], ROW_HOVER);
    theme.menu_hover = reach(t.menu_hover, t.text, &[t.menu_surface], CONTROL);
}
