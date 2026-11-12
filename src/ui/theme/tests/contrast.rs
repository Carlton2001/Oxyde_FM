use super::{OxydeTheme, ThemeId};
use gpui::{Hsla, Rgba};

fn luminance(color: Hsla) -> f32 {
    let color: Rgba = color.into();
    let linear = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

fn contrast(foreground: Hsla, background: Hsla) -> f32 {
    let a = luminance(foreground);
    let b = luminance(background);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn checkbox_outline_and_marks_remain_visible_in_every_palette() {
    for id in ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        let outline = contrast(theme.checkbox_border, theme.surface);
        assert!(outline >= 2.0, "{id:?} empty checkbox: {outline:.2}");
        assert!(outline < contrast(theme.text_muted, theme.surface), "{id:?} quiet outline");
        assert!(contrast(theme.text_on_accent, theme.accent) >= 4.5, "{id:?} checkbox mark");
    }
}

#[test]
fn filled_action_labels_remain_readable_in_every_palette() {
    for id in ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        for (label, foreground, background) in [
            ("primary", theme.text_on_accent, theme.accent),
            ("danger", theme.text_on_danger, theme.danger_background),
            ("danger hover", theme.text_on_danger, theme.danger_hover),
        ] {
            let ratio = contrast(foreground, background);
            assert!(ratio >= 4.5, "{id:?} {label}: {ratio:.2}");
        }
    }
}

#[test]
fn secondary_information_remains_readable_on_its_surfaces() {
    for id in ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        for background in [theme.background, theme.active_background, theme.surface,
            theme.sidebar, theme.header, theme.menu_surface, theme.search_banner] {
            let ratio = contrast(theme.text_muted, background);
            assert!(ratio >= 4.5, "{id:?} secondary text: {ratio:.2}");
            assert!(contrast(theme.text, background) >= ratio, "{id:?}: preserve text hierarchy");
        }
    }
}

#[test]
fn success_labels_remain_readable_on_tinted_badges() {
    for id in ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        for surface in [theme.surface, theme.active_background, theme.header] {
            for (tint, opacity) in [(theme.success_text, 0.08), (theme.success_text, 0.10), (theme.toast_success, 0.15)] {
                let background = surface.blend(tint.opacity(opacity));
                let ratio = contrast(theme.success_text, background);
                assert!(ratio >= 4.5, "{id:?} success badge: {ratio:.2}");
            }
        }
    }
}

#[test]
fn structural_borders_and_control_states_stay_perceptible_in_every_palette() {
    use super::legibility::{BORDER, CONTROL, MENU_BORDER, ROW_HOVER};
    for id in ThemeId::ALL {
        let t = OxydeTheme::for_id(id);
        let checks = [
            ("pane border", t.border, vec![t.background, t.active_background, t.sidebar, t.header, t.surface], BORDER),
            ("menu border", t.menu_border, vec![t.menu_surface], MENU_BORDER),
            ("control", t.control_hover, vec![t.surface, t.active_background, t.header, t.menu_surface], CONTROL),
            ("row hover", t.hover, vec![t.background, t.active_background], ROW_HOVER),
            ("menu hover", t.menu_hover, vec![t.menu_surface], CONTROL),
        ];
        for (label, color, surfaces, min) in checks {
            for surface in surfaces {
                let ratio = contrast(color, surface);
                assert!(ratio >= min, "{id:?} {label}: {ratio:.2}");
            }
        }
        // Secondary buttons: pressing must read differently from hovering.
        let hover = t.surface.blend(t.selected);
        let pressed = t.surface.blend(t.selection_hover);
        assert!(contrast(hover, pressed) >= 1.05, "{id:?} secondary press");
    }
}

#[test]
fn scrollbar_thumbs_stand_out_from_their_rail_in_lightness_steps() {
    for id in ThemeId::ALL {
        let t = OxydeTheme::for_id(id);
        for pane in [t.background, t.active_background, t.sidebar, t.surface, t.menu_surface] {
            let rail = pane.blend(t.scrollbar_track);
            let steps = [(t.scrollbar_thumb, 1.6), (t.scrollbar_thumb_hover, 2.2), (t.scrollbar_thumb_drag, 2.8)];
            for (label, (thumb, min)) in ["rest", "hover", "drag"].into_iter().zip(steps) {
                let ratio = contrast(thumb, rail);
                assert!(ratio >= min - 0.01, "{id:?} {label} thumb: {ratio:.2}");
            }
            assert!(contrast(t.scrollbar_thumb_hover, rail) > contrast(t.scrollbar_thumb, rail), "{id:?} hover step");
            assert!(contrast(t.scrollbar_thumb_drag, rail) > contrast(t.scrollbar_thumb_hover, rail), "{id:?} drag step");
        }
    }
}
