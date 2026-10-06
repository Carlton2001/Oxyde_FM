use gpui::Render;
use gpui::*;
pub use oxyde_lib::models::preferences::ThemeId;

#[derive(Debug, Clone, Copy)]
pub struct OxydeTheme {
    pub background: Hsla,        // Fond principal (FileList)
    pub active_background: Hsla, // Fond du panneau actif
    pub sidebar: Hsla,           // Sidebar et panneaux latéraux
    pub header: Hsla,            // TopBar / TabBar
    pub surface: Hsla,           // Éléments de surface (Dropdowns, Modals)
    pub search_banner: Hsla,     // Barre des résultats, distincte de la navigation et des colonnes
    pub border: Hsla,            // Bordures subtiles
    pub text: Hsla,              // Texte principal
    pub text_muted: Hsla,        // Texte secondaire / Headers
    pub accent: Hsla,            // Couleur d'action propre au thème
    pub hover: Hsla,             // État Hover
    pub selected: Hsla,          // État sélectionné / Focus
    pub error: Hsla,             // Messages d'erreur
    pub text_on_accent: Hsla,    // Texte sur une action remplie avec accent
    pub control_hover: Hsla,     // Survol d'une action secondaire
    pub menu_surface: Hsla,
    pub menu_border: Hsla,
    pub menu_hover: Hsla,
    pub icon_muted: Hsla,
    pub icon_disabled: Hsla,
    pub tab_active: Hsla,
    pub tab_close_hover: Hsla,
    pub selection_hover: Hsla,
    pub drag_hover: Hsla,
    pub scrollbar_thumb: Hsla,
    pub scrollbar_thumb_hover: Hsla,
    pub scrollbar_thumb_drag: Hsla,
    pub scrollbar_track_hover: Hsla,
    pub danger_hover: Hsla,
    pub text_on_danger: Hsla,
}

impl Global for OxydeTheme {}

impl OxydeTheme {
    pub fn for_id(id: ThemeId) -> Self {
        match id {
            ThemeId::Dark => Self::dark(),
            ThemeId::Light => Self::light(),
        }
    }

    // Palettes issues des thèmes Tauri oxyde-light.css et oxyde-dark.css.
    // header correspond à surface-secondary ; surface et tab_active à surface-primary.
    // Les sélections gardent le texte normal sur un accent à 20 % (30 % au survol).
    pub fn light() -> Self {
        Self {
            background: rgb(0xfdfbf7).into(),
            active_background: rgb(0xfdfbf7).into(),
            sidebar: rgb(0xf1ede6).into(),
            header: rgb(0xf1ede6).into(),
            surface: rgb(0xfafafa).into(),
            search_banner: rgb(0xf3f0e9).into(),
            border: rgb(0xe6e1d6).into(),
            text: rgb(0x242433).into(),
            text_muted: rgb(0x6e6b85).into(),
            accent: rgb(0x3d5af1).into(),
            hover: rgb(0xe6e2d9).into(),
            selected: rgba(0x3d5af133).into(),
            error: rgb(0xb42335).into(),
            text_on_accent: rgb(0xffffff).into(),
            control_hover: rgb(0xe6e2d9).into(),
            menu_surface: rgb(0xfafafa).into(),
            menu_border: rgb(0xe6e1d6).into(),
            menu_hover: rgb(0xe6e2d9).into(),
            icon_muted: rgb(0x7c7994).into(),
            icon_disabled: rgba(0x7c7994a6).into(),
            tab_active: rgb(0xfafafa).into(),
            tab_close_hover: rgb(0xe6e2d9).into(),
            selection_hover: rgba(0x3d5af14d).into(),
            drag_hover: rgba(0x3d5af126).into(),
            scrollbar_thumb: rgb(0xdcd7cc).into(),
            scrollbar_thumb_hover: rgb(0xcbc6ba).into(),
            scrollbar_thumb_drag: rgb(0xcbc6ba).into(),
            scrollbar_track_hover: rgba(0x6e6b8518).into(),
            danger_hover: rgb(0xc42b1c).into(),
            text_on_danger: rgb(0xffffff).into(),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: rgb(0x12101b).into(),
            active_background: rgb(0x12101b).into(),
            sidebar: rgb(0x08070d).into(),
            header: rgb(0x08070d).into(),
            surface: rgb(0x151320).into(),
            search_banner: rgb(0x201c2c).into(),
            border: rgb(0x232031).into(),
            text: rgb(0xe2e0ed).into(),
            text_muted: rgb(0x8b86a3).into(),
            accent: rgb(0xd4a54a).into(),
            hover: rgb(0x1c1a2b).into(),
            selected: rgba(0xd4a54a33).into(),
            error: rgb(0xf87171).into(),
            text_on_accent: rgb(0x0e0c15).into(),
            control_hover: rgb(0x1c1a2b).into(),
            menu_surface: rgb(0x151320).into(),
            menu_border: rgb(0x2d2a3c).into(),
            menu_hover: rgb(0x1c1a2b).into(),
            icon_muted: rgb(0xa5a2b8).into(),
            icon_disabled: rgba(0xa5a2b8a6).into(),
            tab_active: rgb(0x151320).into(),
            tab_close_hover: rgb(0x1c1a2b).into(),
            selection_hover: rgba(0xd4a54a4d).into(),
            drag_hover: rgba(0xd4a54a26).into(),
            scrollbar_thumb: rgb(0x2d2a3c).into(),
            scrollbar_thumb_hover: rgb(0x3d3a50).into(),
            scrollbar_thumb_drag: rgb(0x3d3a50).into(),
            scrollbar_track_hover: rgba(0x8b86a318).into(),
            danger_hover: rgb(0xc42b1c).into(),
            text_on_danger: rgb(0xffffff).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThemeId;

    #[test]
    fn theme_preference_has_a_stable_identifier() {
        assert_eq!(
            serde_json::to_string(&ThemeId::default()).unwrap(),
            "\"dark\""
        );
        assert_eq!(
            serde_json::from_str::<ThemeId>("\"dark\"").unwrap(),
            ThemeId::Dark
        );
    }
}

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
