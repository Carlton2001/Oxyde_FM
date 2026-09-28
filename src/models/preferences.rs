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
