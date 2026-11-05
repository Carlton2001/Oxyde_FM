//! User themes: six seed colors saved in the configuration, and the theme selection.

use crate::models::preferences::ThemeId;
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "custom_theme/tests/colors.rs"]
mod tests;

/// Prefix of a user theme in `AppConfig::theme`; older versions read it as the default theme.
const CUSTOM_PREFIX: &str = "custom:";

/// A theme created by the user. Colors are `#rrggbb` so the configuration stays readable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomTheme {
    pub id: String,
    pub name: String,
    /// Built-in theme it started from; it also supplies the file icon colors.
    pub base: String,
    pub background: String,
    pub active_background: String,
    pub sidebar: String,
    pub surface: String,
    pub text: String,
    pub accent: String,
}

impl CustomTheme {
    pub fn new(id: String, name: String, base: ThemeId, colors: [u32; 6]) -> Self {
        let [background, active_background, sidebar, surface, text, accent] = colors.map(format_hex_color);
        Self { id, name, base: base.as_str().into(), background, active_background, sidebar, surface, text, accent }
    }

    pub fn base(&self) -> ThemeId {
        ThemeId::from_id(&self.base)
    }

    /// Seeds in the order background, active background, sidebar, surface, text, accent.
    /// A field edited by hand into an invalid value keeps the base theme's color.
    pub fn colors(&self, fallback: [u32; 6]) -> [u32; 6] {
        let fields = [&self.background, &self.active_background, &self.sidebar, &self.surface, &self.text, &self.accent];
        std::array::from_fn(|index| parse_hex_color(fields[index]).unwrap_or(fallback[index]))
    }

    /// Next free identifier, stable across renames.
    pub fn next_id(existing: &[CustomTheme]) -> String {
        let next = existing.iter()
            .filter_map(|theme| theme.id.strip_prefix("theme-")?.parse::<u32>().ok())
            .max()
            .map_or(1, |last| last + 1);
        format!("theme-{next}")
    }
}

/// The selected theme: built-in, or a user theme by identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeChoice {
    Builtin(ThemeId),
    Custom(String),
}

impl Default for ThemeChoice {
    fn default() -> Self {
        Self::Builtin(ThemeId::default())
    }
}

impl ThemeChoice {
    /// A user theme that no longer exists falls back to the default theme.
    pub fn from_config(value: &str, custom: &[CustomTheme]) -> Self {
        match value.strip_prefix(CUSTOM_PREFIX) {
            Some(id) if custom.iter().any(|theme| theme.id == id) => Self::Custom(id.into()),
            Some(_) => Self::default(),
            None => Self::Builtin(ThemeId::from_id(value)),
        }
    }

    pub fn to_config(&self) -> String {
        match self {
            Self::Builtin(id) => id.as_str().into(),
            Self::Custom(id) => format!("{CUSTOM_PREFIX}{id}"),
        }
    }
}

/// Accepts `#rrggbb`, `#rgb`, with or without `#`, in any case, surrounded by spaces.
pub fn parse_hex_color(value: &str) -> Option<u32> {
    let digits = value.trim();
    let digits = digits.strip_prefix('#').unwrap_or(digits);
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        6 => u32::from_str_radix(digits, 16).ok(),
        3 => {
            let short = u32::from_str_radix(digits, 16).ok()?;
            let (r, g, b) = ((short >> 8) & 0xf, (short >> 4) & 0xf, short & 0xf);
            Some(((r * 0x11) << 16) | ((g * 0x11) << 8) | (b * 0x11))
        }
        _ => None,
    }
}

pub fn format_hex_color(color: u32) -> String {
    format!("#{:06x}", color & 0xff_ffff)
}
