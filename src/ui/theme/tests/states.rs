use super::super::{ColorVision, OxydeTheme, ThemeId};
use super::{chroma, distance, HOVER_TARGET, BAR_FROM_PANE, BAR_GAP, BAR_ON_SELECTION, SELECTION_FROM_PANE, SELECTION_GAP};

/// Built-in palettes plus user palettes with a pale, a saturated and a dark accent.
fn palettes() -> Vec<(String, OxydeTheme)> {
    let mut all: Vec<_> = ThemeId::ALL.into_iter().map(|id| (format!("{id:?}"), OxydeTheme::for_id(id))).collect();
    for colors in [
        [0xf4f1ea, 0xfffdf8, 0xeeeae0, 0xffffff, 0x3a3a3a, 0xe8d9a8],
        [0xffffff, 0xffffff, 0xf3f3f3, 0xffffff, 0x111111, 0xff0000],
        [0x101418, 0x151a20, 0x0c1014, 0x1a2027, 0xdde3ea, 0x1e3a8a],
    ] {
        all.push((format!("{colors:x?}"), OxydeTheme::from_colors(colors, ColorVision::Typical)));
    }
    all
}

#[test]
fn rest_hover_histogram_and_selection_stay_apart_in_every_palette() {
    for (name, t) in palettes() {
        for pane in [t.background, t.active_background] {
            let (selected, bar) = (pane.blend(t.selected), pane.blend(t.size_bar));
            let checks = [
                ("selection/pane", distance(selected, pane), SELECTION_FROM_PANE),
                ("selection/hover", distance(selected, t.hover), SELECTION_GAP),
                ("selection/bar", distance(selected, bar), SELECTION_GAP),
                ("hover/bar", distance(t.hover, bar), BAR_GAP),
                ("bar/pane", distance(bar, pane), BAR_FROM_PANE),
                ("bar on selection", distance(selected, selected.blend(t.size_bar)), BAR_ON_SELECTION),
            ];
            for (label, value, min) in checks {
                assert!(value >= min, "{name} {label}: {value:.3} < {min}");
            }
        }
    }
}

#[test]
fn row_hovers_carry_the_accent_in_every_palette() {
    for (name, t) in palettes() {
        let pane = t.active_background;
        let share = chroma(t.hover, pane) / chroma(pane.blend(t.selected), pane);
        assert!(share >= HOVER_TARGET, "{name} hover keeps {share:.2} of the selection's color");
    }
}
