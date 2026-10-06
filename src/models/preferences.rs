use serde::{Deserialize, Serialize};

pub const SEARCH_LIMIT_STEP: u32 = 500;
pub const MAX_SEARCH_LIMIT: u32 = u32::MAX / SEARCH_LIMIT_STEP * SEARCH_LIMIT_STEP;

pub fn normalized_search_limit(value: u32) -> u32 {
    value.clamp(SEARCH_LIMIT_STEP, MAX_SEARCH_LIMIT)
}

pub fn stepped_search_limit(value: u32, increase: bool) -> u32 {
    let value = normalized_search_limit(value);
    if increase {
        value.saturating_add(SEARCH_LIMIT_STEP).min(MAX_SEARCH_LIMIT)
    } else {
        value.saturating_sub(SEARCH_LIMIT_STEP).max(SEARCH_LIMIT_STEP)
    }
}

#[cfg(test)]
mod search_limit_tests {
    use super::*;

    #[test]
    fn search_limit_steps_are_bounded_and_handle_legacy_values() {
        assert_eq!(stepped_search_limit(3000, true), 3500);
        assert_eq!(stepped_search_limit(3000, false), 2500);
        assert_eq!(stepped_search_limit(500, false), 500);
        assert_eq!(stepped_search_limit(0, false), 500);
        assert_eq!(normalized_search_limit(123), 500);
        assert_eq!(normalized_search_limit(3200), 3200);
        assert_eq!(stepped_search_limit(3200, true), 3700);
        assert_eq!(stepped_search_limit(u32::MAX, true), MAX_SEARCH_LIMIT);
        assert_eq!(stepped_search_limit(MAX_SEARCH_LIMIT, false), MAX_SEARCH_LIMIT - 500);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeId {
    #[default]
    #[serde(alias = "oxyde-dark")]
    Dark,
    #[serde(alias = "oxyde-light")]
    Light,
    AyuLight,
    AyuDark,
    GithubLight,
    GithubDark,
    Monokai,
    OneLight,
    OneDark,
    SolarizedLight,
    SolarizedDark,
    WindowsLight,
    WindowsDark,
}

impl ThemeId {
    pub const ALL: [Self; 13] = [
        Self::Light, Self::Dark,
        Self::AyuLight, Self::AyuDark,
        Self::GithubLight, Self::GithubDark,
        Self::Monokai,
        Self::OneLight, Self::OneDark,
        Self::SolarizedLight, Self::SolarizedDark,
        Self::WindowsLight, Self::WindowsDark,
    ];

    pub fn is_light(self) -> bool {
        matches!(self, Self::Light | Self::AyuLight | Self::GithubLight | Self::OneLight | Self::SolarizedLight | Self::WindowsLight)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Light | Self::Dark => "Oxyde",
            Self::AyuLight | Self::AyuDark => "Ayu",
            Self::GithubLight | Self::GithubDark => "GitHub",
            Self::Monokai => "Monokai",
            Self::OneLight | Self::OneDark => "One",
            Self::SolarizedLight | Self::SolarizedDark => "Solarized",
            Self::WindowsLight | Self::WindowsDark => "Windows",
        }
    }
    pub fn from_id(id: &str) -> Self {
        match id {
            "light" | "oxyde-light" => Self::Light,
            "ayu-light" => Self::AyuLight,
            "ayu-dark" => Self::AyuDark,
            "github-light" => Self::GithubLight,
            "github-dark" => Self::GithubDark,
            "monokai" => Self::Monokai,
            "one-light" => Self::OneLight,
            "one-dark" => Self::OneDark,
            "solarized-light" => Self::SolarizedLight,
            "solarized-dark" => Self::SolarizedDark,
            "windows-light" => Self::WindowsLight,
            "windows-dark" => Self::WindowsDark,
            _ => Self::Dark,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::AyuLight => "ayu-light",
            Self::AyuDark => "ayu-dark",
            Self::GithubLight => "github-light",
            Self::GithubDark => "github-dark",
            Self::Monokai => "monokai",
            Self::OneLight => "one-light",
            Self::OneDark => "one-dark",
            Self::SolarizedLight => "solarized-light",
            Self::SolarizedDark => "solarized-dark",
            Self::WindowsLight => "windows-light",
            Self::WindowsDark => "windows-dark",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "en")]
    English,
}

impl Language {
    /// Unsupported and empty identifiers use the initial French UI language.
    pub fn from_id(id: &str) -> Self {
        match id
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "en" => Self::English,
            _ => Self::French,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::French => "fr",
            Self::English => "en",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    Us,
    #[default]
    European,
    Iso,
}
impl DateFormat {
    pub fn from_id(id: &str) -> Option<Self> {
        match id.to_ascii_lowercase().as_str() {
            "us" => Some(Self::Us),
            "european" => Some(Self::European),
            "iso" | "iso8601" | "iso 8601" => Some(Self::Iso),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Us => "US",
            Self::European => "European",
            Self::Iso => "ISO",
        }
    }
    pub fn for_locale(locale: &str) -> Self {
        if locale
            .split('.')
            .next()
            .unwrap_or("")
            .replace('_', "-")
            .eq_ignore_ascii_case("en-US")
        {
            Self::Us
        } else {
            Self::European
        }
    }
}

#[cfg(test)]
mod date_format_tests {
    use super::DateFormat;
    #[test]
    fn only_us_english_defaults_to_us() {
        for locale in ["en-US", "en_US.UTF-8", "EN-us"] {
            assert_eq!(DateFormat::for_locale(locale), DateFormat::Us);
        }
        for locale in ["en-GB", "en-CA", "en-AU", "fr-FR", "en", "", "C"] {
            assert_eq!(DateFormat::for_locale(locale), DateFormat::European);
        }
        for format in [DateFormat::Us, DateFormat::European, DateFormat::Iso] {
            assert_eq!(DateFormat::from_id(format.as_str()), Some(format));
        }
        assert_eq!(DateFormat::from_id("unknown"), None);
    }
}
