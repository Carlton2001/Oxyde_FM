use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeId {
    #[default]
    #[serde(alias = "oxyde-dark")]
    Dark,
    #[serde(alias = "oxyde-light")]
    Light,
}

impl ThemeId {
    pub fn from_id(id: &str) -> Self {
        match id {
            "light" | "oxyde-light" => Self::Light,
            _ => Self::Dark,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
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
