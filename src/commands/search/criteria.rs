use glob::Pattern;
use regex::Regex;

#[derive(Clone)]
pub(super) enum SearchPattern {
    Glob(Pattern, bool, bool),
    Regex(Regex, bool),
    Literal(String, bool, bool),
}

impl SearchPattern {
    pub(super) fn matches(&self, text: &str) -> bool {
        match self {
            Self::Glob(pattern, ignore_accents, case_sensitive) => {
                let target = normalize(text, *ignore_accents);
                let target = if *case_sensitive {
                    target
                } else {
                    target.to_lowercase()
                };
                pattern.matches_with(
                    &target,
                    glob::MatchOptions {
                        case_sensitive: *case_sensitive,
                        require_literal_separator: false,
                        require_literal_leading_dot: false,
                    },
                )
            }
            Self::Regex(pattern, ignore_accents) => {
                if *ignore_accents {
                    pattern.is_match(&normalize(text, true))
                } else {
                    pattern.is_match(text)
                }
            }
            Self::Literal(query, case_sensitive, ignore_accents) => {
                let target = normalize(text, *ignore_accents);
                let query = normalize(query, *ignore_accents);
                if *case_sensitive {
                    target.contains(&query)
                } else {
                    target.to_lowercase().contains(&query.to_lowercase())
                }
            }
        }
    }
}

fn normalize(text: &str, ignore_accents: bool) -> String {
    if ignore_accents {
        crate::utils::remove_accents(text)
    } else {
        text.to_owned()
    }
}

pub(super) struct SearchParams {
    pub(super) pattern: SearchPattern,
    pub(super) min_size: Option<u64>,
    pub(super) max_size: Option<u64>,
    pub(super) min_date: Option<u64>,
    pub(super) max_date: Option<u64>,
    pub(super) content: Option<Regex>,
}

impl SearchParams {
    pub(super) fn has_date_filter(&self) -> bool {
        self.min_date.is_some() || self.max_date.is_some()
    }

    /// A directory has no file size/content. An unavailable date cannot prove a match.
    pub(super) fn matches_metadata(
        &self,
        name: &str,
        is_dir: bool,
        size: u64,
        modified: Option<u64>,
    ) -> bool {
        self.pattern.matches(name)
            && (!is_dir
                || (self.min_size.is_none() && self.max_size.is_none() && self.content.is_none()))
            && self.min_size.is_none_or(|min| size >= min)
            && self.max_size.is_none_or(|max| size <= max)
            && (!self.has_date_filter()
                || modified.is_some_and(|date| {
                    self.min_date.is_none_or(|min| date >= min)
                        && self.max_date.is_none_or(|max| date <= max)
                }))
    }
}

/// Stable reasons carried by the existing archive-warning stream and translated by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveSearchLimitation {
    ContentNotSearched,
    DatesUnavailable,
}

impl ArchiveSearchLimitation {
    pub fn message(self) -> &'static str {
        match self {
            Self::ContentNotSearched => "Archive search limitation [content]",
            Self::DatesUnavailable => "Archive search limitation [dates]",
        }
    }

    pub fn from_message(message: &str) -> Option<Self> {
        [Self::ContentNotSearched, Self::DatesUnavailable]
            .into_iter()
            .find(|reason| {
                message == reason.message()
                    || message.strip_prefix("Archive Error: ") == Some(reason.message())
            })
    }
}

#[cfg(test)]
#[path = "tests/predicate_tests.rs"]
mod tests;
