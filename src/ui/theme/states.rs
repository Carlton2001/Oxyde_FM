//! Interaction fills that must stay apart from each other in every palette: rest, hover,
//! size histogram and selection. Distances are measured in Oklab, where equal steps look
//! equal whatever the hue or lightness. Selection and histogram keep the accent hue and
//! only change strength; hovers take a hint of the accent and give it up when it would
//! bring them too close to the selection or the histogram.

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
/// Usual histogram strength, raised only where the accent is too pale to show.
const BAR_SHARE: f32 = 0.1;
/// Largest accent share in a hover.
const HOVER_TINT: f32 = 0.1;
/// How much heavier than the neutral hover a tinted one may be.
const HOVER_SLACK: f32 = 0.01;

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

/// Opacity steps of one percent from `from` to `to`.
fn shares(from: f32, to: f32) -> impl DoubleEndedIterator<Item = f32> {
    ((from * 100.0).round() as u32..=(to * 100.0).round() as u32).map(|step| step as f32 / 100.0)
}

/// Lightest fill over `panes`, with `tint` of accent, that reaches `min` contrast toward the text.
fn legible(panes: &[Hsla], text: Hsla, accent: Hsla, tint: f32, min: f32) -> Hsla {
    let base = panes[panes.len() - 1].blend(accent.opacity(tint));
    shares(0.0, 0.6)
        .map(|share| base.blend(text.opacity(share)))
        .find(|c| panes.iter().all(|pane| contrast(*c, *pane) >= min))
        .unwrap_or(text)
}

/// Hover fill over `panes`: the strongest accent hint that keeps it legible, no heavier than
/// the neutral hover, and apart from every fill in `avoid`; otherwise the neutral hover.
fn hover(panes: &[Hsla], text: Hsla, accent: Hsla, min: f32, avoid: &[(Hsla, f32)]) -> Hsla {
    let neutral = legible(panes, text, accent, 0.0, min);
    let base = panes[panes.len() - 1];
    let weight = distance(neutral, base) + HOVER_SLACK;
    let apart = |c: Hsla| panes.iter().all(|pane| avoid.iter().all(|(fill, gap)| distance(c, pane.blend(*fill)) >= *gap));
    shares(0.01, HOVER_TINT)
        .rev()
        .map(|tint| legible(panes, text, accent, tint, min))
        .find(|c| distance(*c, base) <= weight && apart(*c))
        .unwrap_or(neutral)
}

/// Selection and histogram strengths over the row panes for one accent, if any fit. The
/// selection and the bar also clear the neutral `hover`, so a hover always fits between them.
fn accent_fills(accent: Hsla, rows: &[Hsla], hover: Hsla) -> Option<(f32, f32)> {
    let on = |pane: Hsla, share: f32| pane.blend(accent.opacity(share));
    // Closest strength to the usual one that shows on the pane and differs from the hover.
    let mut strengths: Vec<f32> = shares(0.04, 0.3).collect();
    strengths.sort_by(|a, b| (a - BAR_SHARE).abs().total_cmp(&(b - BAR_SHARE).abs()));
    let bar = strengths.into_iter().find(|b| {
        rows.iter().all(|pane| distance(*pane, on(*pane, *b)) >= BAR_FROM_PANE && distance(hover, on(*pane, *b)) >= BAR_GAP)
    })?;
    let selected = shares(0.2, 0.6).find(|s| {
        rows.iter().all(|pane| {
            let sel = on(*pane, *s);
            distance(*pane, sel) >= SELECTION_FROM_PANE
                && distance(sel, hover) >= SELECTION_GAP + HOVER_SLACK
                && distance(sel, on(*pane, bar)) >= SELECTION_GAP
                && distance(sel, sel.blend(accent.opacity(bar))) >= BAR_ON_SELECTION
        })
    })?;
    Some((bar, selected))
}

pub(super) fn derive(theme: &mut OxydeTheme) {
    let t = *theme;
    let (accent, text) = (t.accent, t.text);
    let rows = [t.background, t.active_background];
    let neutral = legible(&rows, text, accent, 0.0, ROW_HOVER);
    // A pale accent cannot mark a selection on its own: it then leans toward the text.
    let (fill, (bar, selected)) = shares(0.0, 1.0)
        .step_by(10)
        .map(|share| accent.blend(text.opacity(share)))
        .find_map(|fill| accent_fills(fill, &rows, neutral).map(|shares| (fill, shares)))
        .unwrap_or((text, (0.06, 0.2)));
    theme.size_bar = fill.opacity(bar);
    theme.selected = fill.opacity(selected);
    theme.selection_hover = fill.opacity(selected + 0.1);
    let row_avoid = [(theme.selected, SELECTION_GAP), (theme.size_bar, BAR_GAP)];
    theme.hover = hover(&rows, text, accent, ROW_HOVER, &row_avoid);
    let control_avoid = [(theme.selected, SELECTION_GAP)];
    let holders = [t.active_background, t.header, t.menu_surface, t.surface];
    theme.control_hover = hover(&holders, text, accent, CONTROL, &control_avoid);
    theme.menu_hover = hover(&[t.menu_surface], text, accent, CONTROL, &control_avoid);
    theme.tab_close_hover = hover(&[t.surface], text, accent, 1.45, &[]);
}
