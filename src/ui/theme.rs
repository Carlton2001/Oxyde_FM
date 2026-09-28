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
    pub border: Hsla,            // Bordures subtiles
    pub text: Hsla,              // Texte principal
    pub text_muted: Hsla,        // Texte secondaire / Headers
    pub accent: Hsla,            // Couleur d'action (Blue Zed-like)
    pub hover: Hsla,             // État Hover
    pub selected: Hsla,          // État sélectionné / Focus
    pub modal_backdrop: Hsla,    // Fond qui recouvre l'interface sous une modale
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

    pub fn light() -> Self {
        Self {
            background: rgb(0xf6f7f9).into(),
            active_background: rgb(0xffffff).into(),
            sidebar: rgb(0xedeff3).into(),
            header: rgb(0xedeff3).into(),
            surface: rgb(0xffffff).into(),
            border: rgb(0xcbd0d9).into(),
            text: rgb(0x202733).into(),
            text_muted: rgb(0x596273).into(),
            accent: rgb(0x0068bb).into(),
            hover: rgba(0x0068bb0c).into(),
            selected: rgba(0x0068bb20).into(),
            modal_backdrop: rgba(0x17223466).into(),
            error: rgb(0xb42335).into(),
            text_on_accent: rgb(0xffffff).into(),
            control_hover: rgb(0xe9edf3).into(),
            menu_surface: rgb(0xffffff).into(),
            menu_border: rgb(0xcbd0d9).into(),
            menu_hover: rgb(0xe9edf3).into(),
            icon_muted: rgb(0x667085).into(),
            icon_disabled: rgb(0xa0a7b3).into(),
            tab_active: rgb(0xffffff).into(),
            tab_close_hover: rgb(0xdfe5ee).into(),
            selection_hover: rgba(0x0068bb35).into(),
            drag_hover: rgba(0x0068bb35).into(),
            scrollbar_thumb: rgba(0x59627366).into(),
            scrollbar_thumb_hover: rgb(0x657083).into(),
            scrollbar_thumb_drag: rgb(0x384456).into(),
            scrollbar_track_hover: rgba(0x59627318).into(),
            danger_hover: rgb(0xc42b1c).into(),
            text_on_danger: rgb(0xffffff).into(),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: rgb(0x0c0c0c).into(),
            active_background: rgb(0x161719).into(),
            sidebar: rgb(0x1f2023).into(),
            header: rgb(0x1f2023).into(),
            surface: rgb(0x38393d).into(),
            border: rgb(0x161719).into(),
            text: rgb(0xcccccc).into(),
            text_muted: rgb(0x999999).into(),
            accent: rgb(0x0078d4).into(),
            hover: rgba(0xffffff05).into(),
            selected: rgba(0x0078d420).into(),
            modal_backdrop: rgba(0x00000088).into(),
            error: rgb(0xe06c75).into(),
            text_on_accent: rgb(0xffffff).into(),
            control_hover: rgb(0x1f2023).into(),
            menu_surface: rgb(0x1e1f23).into(),
            menu_border: rgba(0xffffff14).into(),
            menu_hover: rgb(0x30343b).into(),
            icon_muted: rgb(0x888888).into(),
            icon_disabled: rgba(0xffffff33).into(),
            tab_active: rgb(0x38393d).into(),
            tab_close_hover: rgb(0x333333).into(),
            selection_hover: rgba(0x0078d440).into(),
            drag_hover: rgba(0x0078d440).into(),
            scrollbar_thumb: rgba(0x55555544).into(),
            scrollbar_thumb_hover: rgb(0xaaaaaa).into(),
            scrollbar_thumb_drag: rgb(0xffffff).into(),
            scrollbar_track_hover: rgba(0xffffff11).into(),
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
