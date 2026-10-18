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
            theme.sidebar, theme.header, theme.menu_surface, theme.search_banner, theme.dialog_tabs] {
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
