//! What a column filter keeps beyond its checked values: custom ranges and the way a
//! typed name is matched.
use chrono::NaiveDate;

/// A details column that can be filtered: clearing one, and counts that ignore its own filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FilterColumn {
    Name,
    Location,
    Type,
    Size,
    Permissions,
    Date,
    /// When an entry went to the trash.
    Deleted,
}

/// One checkable value of a column filter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FilterValue {
    /// A label of `file_type_label`.
    Type(String),
    Size(super::SizeCategory),
    Date(crate::ui::date_groups::DateGroup),
    Deleted(crate::ui::date_groups::DateGroup),
    Permissions(u32),
}

/// Sizes in bytes, both bounds included; a missing bound leaves that side open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SizeRange {
    pub min: Option<u64>,
    pub max: Option<u64>,
}

impl SizeRange {
    pub fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    pub fn contains(&self, size: u64) -> bool {
        self.min.is_none_or(|min| size >= min) && self.max.is_none_or(|max| size <= max)
    }
}

/// Local calendar days, both included; a missing bound leaves that side open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DateRange {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

impl DateRange {
    pub fn is_open(&self) -> bool {
        self.from.is_none() && self.to.is_none()
    }

    /// Whether the local day of `ms`, milliseconds since the epoch, lies in the range.
    pub fn contains(&self, ms: u64) -> bool {
        let Some(day) = i64::try_from(ms)
            .ok()
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(|date| date.with_timezone(&chrono::Local).date_naive())
        else {
            return false;
        };
        self.from.is_none_or(|from| day >= from) && self.to.is_none_or(|to| day <= to)
    }
}

/// How the name filter compares: a part of the name, a whole-name pattern once it holds
/// `*` or `?`, or a regular expression. Case never matters. An expression that does not
/// compile filters nothing, its field showing the error.
pub(crate) enum NameMatcher {
    All,
    Contains(String),
    Glob(glob::Pattern),
    Regex(regex::Regex),
}

impl NameMatcher {
    pub(crate) fn new(text: &str, regex: bool) -> Self {
        if text.is_empty() {
            return Self::All;
        }
        if regex {
            return regex::RegexBuilder::new(text)
                .case_insensitive(true)
                .build()
                .map_or(Self::All, Self::Regex);
        }
        if text.contains(['*', '?']) {
            return glob::Pattern::new(&text.to_lowercase()).map_or(Self::All, Self::Glob);
        }
        Self::Contains(text.to_lowercase())
    }

    /// Whether `text` compiles in that mode: a plain text always does.
    pub(crate) fn is_valid(text: &str, regex: bool) -> bool {
        text.is_empty() || !matches!(Self::new(text, regex), Self::All)
    }

    pub(crate) fn matches(&self, name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Contains(part) => name.to_lowercase().contains(part.as_str()),
            Self::Glob(pattern) => pattern.matches(&name.to_lowercase()),
            Self::Regex(regex) => regex.is_match(name),
        }
    }
}
