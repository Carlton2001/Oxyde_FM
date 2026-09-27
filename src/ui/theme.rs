use gpui::Render;
use gpui::*;

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
}

impl Global for OxydeTheme {}

impl OxydeTheme {
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
        }
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

pub trait PixelsExt {
    fn as_f32(&self) -> f32;
}

impl PixelsExt for Pixels {
    fn as_f32(&self) -> f32 {
        self.to_f64() as f32
    }
}
