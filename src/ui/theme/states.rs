//! Interaction fills that must stay apart from each other in every palette: rest, hover,
//! size histogram and selection. Distances are measured in Oklab, where equal steps look
//! equal whatever the hue or lightness. Selection and histogram keep the accent hue and
//! only change strength. Hovers keep the contrast of a neutral hover and add accent, up to
//! half the selection's colorfulness, while keeping clear of the selection and the histogram.

use super::legibility::{contrast, CONTROL, ROW_HOVER};
use super::OxydeTheme;
use gpui::{Hsla, Rgba};

#[cfg(test)]
#[path = "tests/states.rs"]
mod tests;

/// Selection against its pane.
pub(super) const SELECTION_FROM_PANE: f32 = 0.1;
/// Histogram bar against its pane.
pub(super) const BAR_FROM_PANE: f32 = 0.025;
/// Histogram bar over a selection: the selection already marks the row, the bar stays faint.
pub(super) const BAR_ON_SELECTION: f32 = 0.01;
/// Selection against a hover or a histogram bar.
pub(super) const SELECTION_GAP: f32 = 0.05;
/// Hover against a histogram bar.
pub(super) const BAR_GAP: f32 = 0.03;
/// Share of the selection's colorfulness a hover may carry.
const HOVER_CHROMA: f32 = 0.5;
/// Usual histogram strength, fainter than the hover so a partial bar never reads as one.
const BAR_SHARE: f32 = 0.08;
/// Colorfulness a row hover must reach, as a share of the selection's; the selection
/// strengthens and the histogram strays from its usual strength to make room for it.
pub(super) const HOVER_TARGET: f32 = 0.35;
/// Strongest histogram bar.
const BAR_MAX: f32 = 0.2;
/// Largest accent share in a hover.
const HOVER_TINT: f32 = 0.5;

fn oklab(color: Hsla) -> [f32; 3] {
    let color: Rgba = color.into();
    let linear = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let (r, g, b) = (linear(color.r), linear(color.g), linear(color.b));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// Perceptual distance between two opaque colors; about 0.02 is a just noticeable step.
pub(super) fn distance(a: Hsla, b: Hsla) -> f32 {
    let (a, b) = (oklab(a), oklab(b));
    a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f32>().sqrt()
}

/// Colorfulness of `color` relative to `pane`: the Oklab distance across hue and chroma only.
pub(super) fn chroma(color: Hsla, pane: Hsla) -> f32 {
    let (c, p) = (oklab(color), oklab(pane));
    ((c[1] - p[1]).powi(2) + (c[2] - p[2]).powi(2)).sqrt()
}

/// Opacity steps of one percent from `from` to `to`.
fn shares(from: f32, to: f32) -> impl DoubleEndedIterator<Item = f32> {
    ((from * 100.0).round() as u32..=(to * 100.0).round() as u32).map(|step| step as f32 / 100.0)
}

/// Neutral hover over `panes`: the lightest shift toward the text reaching `min` contrast.
fn neutral(panes: &[Hsla], text: Hsla, min: f32) -> Hsla {
    let base = panes[panes.len() - 1];
    shares(0.0, 0.6)
        .map(|share| base.blend(text.opacity(share)))
        .find(|c| panes.iter().all(|pane| contrast(*c, *pane) >= min))
        .unwrap_or(text)
}

/// Hover over `panes` with `tint` of accent: the lightest shift toward the text that keeps
/// the `min` contrast and stands out at least as much as the neutral hover.
fn tinted(panes: &[Hsla], text: Hsla, accent: Hsla, tint: f32, min: f32, reach: f32) -> Option<Hsla> {
    let base = panes[panes.len() - 1].blend(accent.opacity(tint));
    shares(0.0, 0.6)
        .map(|share| base.blend(text.opacity(share)))
        .find(|c| panes.iter().all(|pane| contrast(*c, *pane) >= min && distance(*c, *pane) >= reach))
}

/// Hover over `panes`: the most accent that stays within `HOVER_CHROMA` of the selection's
/// colorfulness and apart from every fill in `avoid`, or `None` when even the neutral hover
/// is too close to one of them.
fn hover(panes: &[Hsla], text: Hsla, accent: Hsla, min: f32, selected: Hsla, avoid: &[(Hsla, f32)]) -> Option<Hsla> {
    let plain = neutral(panes, text, min);
    let base = panes[panes.len() - 1];
    let reach = panes.iter().map(|pane| distance(plain, *pane)).fold(f32::INFINITY, f32::min);
    let ceiling = HOVER_CHROMA * chroma(base.blend(selected), base);
    let apart = |c: Hsla| panes.iter().all(|pane| avoid.iter().all(|(fill, gap)| distance(c, pane.blend(*fill)) >= *gap));
    shares(0.01, HOVER_TINT)
        .rev()
        .filter_map(|tint| tinted(panes, text, accent, tint, min, reach))
        .chain([plain])
        .find(|c| chroma(*c, base) <= ceiling && apart(*c))
}

/// Whether a selection of `selected` and a histogram of `bar` show on the row panes and
/// stay apart from each other.
fn fits(accent: Hsla, rows: &[Hsla], bar: f32, selected: f32) -> bool {
    let on = |pane: Hsla, share: f32| pane.blend(accent.opacity(share));
    rows.iter().all(|pane| {
        let sel = on(*pane, selected);
        distance(*pane, on(*pane, bar)) >= BAR_FROM_PANE
            && distance(*pane, sel) >= SELECTION_FROM_PANE
            && distance(sel, on(*pane, bar)) >= SELECTION_GAP
            && distance(sel, sel.blend(accent.opacity(bar))) >= BAR_ON_SELECTION
    })
}

/// Histogram, selection and row hover for one accent: the weakest selection, then the
/// histogram nearest its usual strength, that leave the hover `HOVER_TARGET` of the
/// selection's colorfulness; failing that, the pair leaving the most colorful hover.
fn row_fills(accent: Hsla, rows: &[Hsla], text: Hsla) -> Option<(f32, f32, Hsla)> {
    let mut bars: Vec<f32> = shares(0.04, BAR_MAX).collect();
    bars.sort_by(|a, b| (a - BAR_SHARE).abs().total_cmp(&(b - BAR_SHARE).abs()));
    let base = rows[rows.len() - 1];
    let mut best: Option<(f32, f32, Hsla, f32)> = None;
    for selected in shares(0.2, 0.6) {
        let ceiling = chroma(base.blend(accent.opacity(selected)), base);
        for &bar in bars.iter().filter(|bar| fits(accent, rows, **bar, selected)) {
            let avoid = [(accent.opacity(selected), SELECTION_GAP), (accent.opacity(bar), BAR_GAP)];
            let Some(fill) = hover(rows, text, accent, ROW_HOVER, accent.opacity(selected), &avoid) else { continue };
            let share = chroma(fill, base) / ceiling;
            if share >= HOVER_TARGET {
                return Some((bar, selected, fill));
            }
            if best.is_none_or(|(.., kept)| share > kept + 0.02) {
                best = Some((bar, selected, fill, share));
            }
        }
    }
    best.map(|(bar, selected, fill, _)| (bar, selected, fill))
}

pub(super) fn derive(theme: &mut OxydeTheme) {
    let t = *theme;
    let text = t.text;
    let rows = [t.background, t.active_background];
    // A pale accent cannot mark a selection on its own: it then leans toward the text.
    let (fill, (bar, selected, row_hover)) = shares(0.0, 1.0)
        .step_by(10)
        .map(|share| t.accent.blend(text.opacity(share)))
        .find_map(|fill| row_fills(fill, &rows, text).map(|fills| (fill, fills)))
        .unwrap_or((text, (0.06, 0.2, neutral(&rows, text, ROW_HOVER))));
    theme.size_bar = fill.opacity(bar);
    theme.selected = fill.opacity(selected);
    theme.selection_hover = fill.opacity(selected + 0.1);
    theme.hover = row_hover;
    let sel = theme.selected;
    let avoid = [(sel, SELECTION_GAP)];
    let holders = [t.active_background, t.header, t.menu_surface, t.surface];
    let other = |panes: &[Hsla], min: f32, avoid: &[(Hsla, f32)]| hover(panes, text, fill, min, sel, avoid).unwrap_or_else(|| neutral(panes, text, min));
    theme.control_hover = other(&holders, CONTROL, &avoid);
    theme.menu_hover = other(&[t.menu_surface], CONTROL, &avoid);
    theme.tab_close_hover = other(&[t.surface], 1.45, &[]);
}
