use gpui::Render;
mod file_colors;
mod legibility;
mod seed;
pub(crate) use seed::ThemeSeed;
pub use oxyde_lib::models::custom_theme::{CustomTheme, ThemeChoice};
use gpui::*;
pub use oxyde_lib::models::preferences::{ColorVision, ThemeId};

#[derive(Debug, Clone, Copy)]
pub struct OxydeTheme {
    /// Light background: picks status colors and light-variant assets.
    pub light: bool,
    pub background: Hsla,        // Fond des panneaux de fichiers inactifs
    pub active_background: Hsla, // Fond du panneau actif
    pub sidebar: Hsla,           // Sidebar et panneaux latéraux
    pub header: Hsla,            // TopBar / TabBar
    pub surface: Hsla,           // Éléments de surface (Dropdowns, Modals)
    pub search_banner: Hsla,     // Barre des résultats, distincte de la navigation et des colonnes
    pub border: Hsla,            // Bordures subtiles
    pub checkbox_border: Hsla,   // Contour neutre des cases vides
    pub text: Hsla,              // Texte principal
    pub text_muted: Hsla,        // Texte secondaire / Headers
    pub accent: Hsla,            // Couleur d'action propre au thème
    pub hover: Hsla,             // État Hover
    pub selected: Hsla,          // État sélectionné / Focus
    pub media_ssd: Hsla,
    pub media_hdd: Hsla,
    pub media_removable: Hsla,
    pub media_network: Hsla,
    pub text_on_badge: Hsla,
    pub badge_surface: Hsla,
    pub warning_border: Hsla,
    pub warning_background: Hsla,
    pub warning_text: Hsla,
    pub toast_success: Hsla,
    pub success_text: Hsla,      // Texte des statuts positifs, y compris sur un badge teinté
    pub toast_error: Hsla,
    pub error: Hsla,             // Messages d'erreur
    pub text_on_accent: Hsla,    // Texte sur une action remplie avec accent
    pub control_hover: Hsla,     // Survol d'une action secondaire
    pub menu_surface: Hsla,
    pub menu_border: Hsla,
    pub menu_hover: Hsla,
    pub file_icons: crate::ui::components::icons::FileIconPalette,
    pub icon_disabled: Hsla,
    pub tab_active: Hsla,
    pub tab_close_hover: Hsla,
    pub selection_hover: Hsla,
    pub drag_hover: Hsla,
    pub scrollbar_thumb: Hsla,
    pub scrollbar_thumb_hover: Hsla,
    pub scrollbar_thumb_drag: Hsla,
    pub scrollbar_track: Hsla,
    pub danger_background: Hsla, // Fond des actions destructives, distinct du texte d’erreur
    pub danger_hover: Hsla,
    pub text_on_danger: Hsla,
}

impl Global for OxydeTheme {}

/// Whether dark text reads better than white text on `background`.
pub(crate) fn is_light_background(background: Hsla) -> bool {
    legibility::contrast(background, black()) >= legibility::contrast(background, white())
}

/// WCAG contrast ratio, shown by the theme editor.
pub(crate) fn contrast(a: Hsla, b: Hsla) -> f32 {
    legibility::contrast(a, b)
}

impl OxydeTheme {
    /// Fond le plus foncé parmi les surfaces du thème, pour la visionneuse.
    pub fn darkest_background(&self) -> Hsla {
        [
            self.background,
            self.active_background,
            self.sidebar,
            self.header,
            self.surface,
            self.search_banner,
            self.menu_surface,
            self.tab_active,
        ]
        .into_iter()
        .min_by(|a, b| a.l.total_cmp(&b.l))
        .unwrap_or(self.background)
    }

    pub fn drive_usage_color(&self, total: u64, free: u64) -> Hsla {
        let used = total.saturating_sub(free);
        if total > 0 && u128::from(used) * 10 >= u128::from(total) * 9 {
            self.error
        } else {
            self.accent
        }
    }

    #[cfg(test)]
    pub fn for_id(id: ThemeId) -> Self {
        Self::for_id_with(id, ColorVision::Typical)
    }

    pub fn for_id_with(id: ThemeId, vision: ColorVision) -> Self {
        Self::from_seed(ThemeSeed::for_id(id), id.is_light(), vision)
    }

    /// A theme from user seeds; brightness comes from the background.
    pub fn from_colors(colors: [u32; 6], vision: ColorVision) -> Self {
        let seed = ThemeSeed::from_colors(colors);
        Self::from_seed(seed, is_light_background(rgb(seed.background).into()), vision)
    }

    /// Every token, file icon colors included, follows from the seeds and the color vision.
    fn from_seed(seed: ThemeSeed, light: bool, vision: ColorVision) -> Self {
        let icons = file_colors::derive([seed.background, seed.active_background], seed.accent, light, vision);
        let mut theme = seed::derive(seed, light, icons, vision);
        legibility::enforce(&mut theme);
        theme
    }

    pub fn for_custom(custom: &CustomTheme, vision: ColorVision) -> Self {
        let base = custom.base();
        Self::from_colors(custom.colors(ThemeSeed::for_id(base).colors()), vision)
    }

    pub fn for_choice(choice: &ThemeChoice, custom: &[CustomTheme], vision: ColorVision) -> Self {
        match choice {
            ThemeChoice::Builtin(id) => Self::for_id_with(*id, vision),
            ThemeChoice::Custom(id) => custom.iter().find(|theme| &theme.id == id)
                .map_or_else(|| Self::for_id_with(ThemeId::default(), vision), |theme| Self::for_custom(theme, vision)),
        }
    }

    #[cfg(test)]
    pub fn light() -> Self {
        Self::for_id(ThemeId::Light)
    }

    #[cfg(test)]
    pub fn dark() -> Self {
        Self::for_id(ThemeId::Dark)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn inactive_panes_have_a_distinct_background_in_every_theme() {
        for id in super::ThemeId::ALL {
            let theme = super::OxydeTheme::for_id(id);
            assert_ne!(theme.background, theme.active_background, "{id:?}");
        }
    }
    use super::ThemeId;

    #[test]
    fn theme_preference_has_a_stable_identifier() {
        for id in ThemeId::ALL {
            assert_eq!(ThemeId::from_id(id.as_str()), id);
        }
        assert!(!ThemeId::Monokai.is_light());
        assert_eq!(ThemeId::default().as_str(), "dark");
        assert_eq!(ThemeId::from_id("dark"), ThemeId::Dark);
        assert_eq!(ThemeId::from_id("oxyde-dark"), ThemeId::Dark);
        assert_eq!(ThemeId::from_id("oxyde-light"), ThemeId::Light);
    }
}

#[cfg(test)]
#[path = "theme/tests/contrast.rs"]
mod contrast_tests;

pub trait ThemeContext {
    fn theme(&self) -> &OxydeTheme;
}

impl ThemeContext for App {
    fn theme(&self) -> &OxydeTheme {
        OxydeTheme::global(self)
    }
}

impl<'a, V: Render> ThemeContext for Context<'a, V> {
    fn theme(&self) -> &OxydeTheme {
        OxydeTheme::global(self)
    }
}
