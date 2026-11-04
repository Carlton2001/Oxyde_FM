//! Builds every theme token from seven input colors.

use super::{legibility, OxydeTheme, ThemeId};
use gpui::{rgb, Hsla};

/// The colors a theme provides; everything else is derived.
#[derive(Debug, Clone, Copy)]
pub(super) struct ThemeSeed {
    pub background: u32,
    pub active_background: u32,
    pub sidebar: u32,
    pub surface: u32,
    pub text: u32,
    pub text_muted: u32,
    pub accent: u32,
}

impl ThemeSeed {
    pub(super) fn for_id(id: ThemeId) -> Self {
        let s = |background, active_background, sidebar, surface, text, text_muted, accent| Self {
            background,
            active_background,
            sidebar,
            surface,
            text,
            text_muted,
            accent,
        };
        match id {
            ThemeId::Light => s(0xf0ede6, 0xfdfbf7, 0xf1ede6, 0xfafafa, 0x242433, 0x6b6882, 0x3d5af1),
            ThemeId::Dark => s(0x12101b, 0x1c1927, 0x08070d, 0x151320, 0xe2e0ed, 0x8b86a3, 0xd4a54a),
            ThemeId::AyuLight => s(0xeef0f2, 0xf8f9fa, 0xf3f4f5, 0xffffff, 0x5c6166, 0x60666d, 0xff9940),
            ThemeId::AyuDark => s(0x0b0e14, 0x10151e, 0x090c10, 0x0f131a, 0xb3b1ad, 0x89919b, 0xe6b450),
            ThemeId::GithubLight => s(0xf0f2f4, 0xffffff, 0xf6f8fa, 0xffffff, 0x1f2328, 0x656d76, 0x0969da),
            ThemeId::GithubDark => s(0x0d1117, 0x161b22, 0x010409, 0x161b22, 0xe6edf3, 0x7d8590, 0x2f81f7),
            ThemeId::Monokai => s(0x272822, 0x32332c, 0x1e1f1c, 0x3e3d32, 0xf8f8f2, 0xaeaa92, 0xfd971f),
            ThemeId::OneLight => s(0xeaeaeb, 0xfafafa, 0xf3f3f3, 0xffffff, 0x383a42, 0x60636d, 0x4078f2),
            ThemeId::OneDark => s(0x21252b, 0x282c34, 0x21252b, 0x2c313a, 0xabb2bf, 0x969fae, 0x61afef),
            ThemeId::SolarizedLight => s(0xeee8d5, 0xfdf6e3, 0xeee8d5, 0xfdf6e3, 0x465c64, 0x556b72, 0xd33682),
            ThemeId::SolarizedDark => s(0x00252f, 0x002b36, 0x001e26, 0x0b3a46, 0xa9b8b8, 0xa0b1b2, 0x268bd2),
            ThemeId::WindowsLight => s(0xeeeeee, 0xfbfbfb, 0xf2f2f2, 0xffffff, 0x2e2e2e, 0x666666, 0x0672cb),
            ThemeId::WindowsDark => s(0x181818, 0x222222, 0x111111, 0x1e1e1e, 0xd0d0d0, 0x999999, 0x52a8ff),
        }
    }
}

/// Smallest blend of `color` toward `toward` reaching `min` against every surface.
fn reach(color: Hsla, toward: Hsla, surfaces: &[Hsla], min: f32) -> Hsla {
    let worst = |c: Hsla| surfaces.iter().map(|s| legibility::contrast(c, *s)).fold(f32::INFINITY, f32::min);
    if worst(color) >= min {
        return color;
    }
    (1..=100)
        .map(|step| color.blend(toward.opacity(step as f32 / 100.0)))
        .find(|candidate| worst(*candidate) >= min)
        .unwrap_or(toward)
}

pub(super) fn derive(id: ThemeId) -> OxydeTheme {
    let seed = ThemeSeed::for_id(id);
    let c = |value: u32| -> Hsla { rgb(value).into() };
    let (background, active_background, sidebar, surface) =
        (c(seed.background), c(seed.active_background), c(seed.sidebar), c(seed.surface));
    let (text, text_muted, accent) = (c(seed.text), c(seed.text_muted), c(seed.accent));
    let panes = [background, active_background, sidebar, surface];
    let thumb_hover = reach(surface, text, &panes, 1.8);
    let (on_dark, on_light): (Hsla, Hsla) = (c(0x0e0c15), c(0xffffff));
    let text_on_accent = if legibility::contrast(on_dark, accent) >= legibility::contrast(on_light, accent) {
        on_dark
    } else {
        on_light
    };
    let light = id.is_light();
    let pick = |l: u32, d: u32| c(if light { l } else { d });
    OxydeTheme {
        background,
        active_background,
        sidebar,
        header: sidebar,
        surface,
        search_banner: surface,
        border: reach(surface, text, &panes, legibility::BORDER),
        checkbox_border: surface.blend(text_muted.opacity(0.65)),
        text,
        text_muted,
        accent,
        hover: reach(active_background, text, &[background, active_background], legibility::ROW_HOVER),
        selected: accent.opacity(0.2),
        media_ssd: c(0x087f5b),
        media_hdd: c(0x2563b7),
        media_removable: c(0x9b6600),
        media_network: c(0x6459bb),
        text_on_badge: c(0xffffff),
        badge_surface: surface,
        warning_border: pick(0xf4e7ad, 0x5a4522),
        warning_background: pick(0xfff8df, 0x302618),
        warning_text: pick(0x915500, 0xf3c56a),
        toast_success: c(0x10b981),
        success_text: pick(0x08613f, 0x49d6a4),
        toast_error: c(0xef4444),
        error: pick(0xb42335, 0xf87171),
        text_on_accent,
        control_hover: reach(surface, text, &[surface, active_background, sidebar], legibility::CONTROL),
        menu_surface: surface,
        menu_border: reach(surface, text, &[surface], legibility::MENU_BORDER),
        menu_hover: reach(surface, text, &[surface], legibility::CONTROL),
        file_icons: crate::ui::components::icons::FileIconPalette::for_theme(id),
        icon_disabled: text_muted.opacity(0.65),
        tab_active: surface,
        tab_close_hover: reach(surface, text, &[surface], 1.45),
        selection_hover: accent.opacity(0.3),
        drag_hover: accent.opacity(0.15),
        scrollbar_thumb: reach(surface, text, &panes, 1.3),
        scrollbar_thumb_hover: thumb_hover,
        scrollbar_thumb_drag: thumb_hover,
        scrollbar_track: text_muted.opacity(0.094),
        danger_background: c(0xb42335),
        danger_hover: c(0xc42b1c),
        text_on_danger: c(0xffffff),
    }
}
