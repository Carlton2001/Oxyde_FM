//! The values a column filter offers: one per value present in the folder, plus those still
//! checked from another folder, each counted among the entries the other filters keep.
use super::presentation::file_type_label;
use crate::ui::components::icons::IconType;
use crate::ui::date_groups::DateGroup;
use crate::ui::i18n::{I18n, Message};
use crate::ui::workspace::{FilterColumn, FilterValue, SizeCategory, TabState};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct FilterEntry {
    pub value: FilterValue,
    pub label: String,
    /// Entries with this value that the other columns' filters keep.
    pub count: u64,
    /// A file of that type, for its icon: Type values only.
    pub sample: Option<(String, IconType)>,
}

/// Kinds that are not extensions, in the order they lead the Type list.
const KINDS: [&str; 6] = ["Dossier", "Fichier", "Symlink", "Junction", "BrokenLink", "Réseau"];

/// The label shown for a Type value: kinds translated, extensions as they are.
pub(crate) fn type_label(value: &str, i18n: &I18n) -> String {
    let message = match value {
        "Dossier" => Message::FileTypeFolder,
        "Fichier" => Message::FileTypeFile,
        "Symlink" => Message::FileTypeSymlink,
        "BrokenLink" => Message::FileTypeBrokenLink,
        "Junction" => Message::FileTypeJunction,
        "Réseau" => Message::SidebarNetwork,
        _ => return value.to_string(),
    };
    i18n.text(message).to_string()
}

/// The values of `column` present in the files, counted among the entries the other
/// filters keep: the part of a panel's entries that walks every file, kept while its
/// [`FacetKey`](crate::ui::workspace::FacetKey) holds.
pub(crate) struct FilterFacets {
    present: Vec<FilterValue>,
    counts: HashMap<FilterValue, u64>,
    samples: HashMap<String, (String, IconType)>,
}

/// The values of `column`, in display order. Text columns have none.
#[cfg(test)]
pub(crate) fn filter_entries(tab: &TabState, column: FilterColumn, i18n: &I18n) -> Vec<FilterEntry> {
    filter_entries_from(tab, column, &filter_facets(tab, column, chrono::Local::now().date_naive()), i18n)
}

pub(crate) fn filter_facets(tab: &TabState, column: FilterColumn, today: chrono::NaiveDate) -> FilterFacets {
    let mut counts = HashMap::<FilterValue, u64>::new();
    let mut present = Vec::<FilterValue>::new();
    let mut samples = HashMap::<String, (String, IconType)>::new();
    let mut seen = std::collections::HashSet::new();
    // One pass: every value present, and those counted among the entries the other filters keep.
    tab.visit_all_faceted(column, today, |file, kept| {
        let Some(value) = value_of(file, column, today) else { return };
        if let FilterValue::Type(label) = &value {
            if !samples.contains_key(label) {
                samples.insert(label.clone(), (file.path.clone(), crate::ui::components::icons::entry_icon(file)));
            }
        }
        if kept {
            *counts.entry(value.clone()).or_default() += 1;
        }
        if seen.insert(value.clone()) {
            present.push(value);
        }
    });
    FilterFacets { present, counts, samples }
}

/// The values of `column`, in display order: those of `facets` and those still checked.
pub(crate) fn filter_entries_from(tab: &TabState, column: FilterColumn, facets: &FilterFacets, i18n: &I18n) -> Vec<FilterEntry> {
    let FilterFacets { present, counts, samples } = facets;
    let mut present = present.clone();
    let mut seen: std::collections::HashSet<&FilterValue> = facets.present.iter().collect();
    // Values checked in another folder stay listed, to be unchecked.
    let checked: Vec<FilterValue> = match column {
        FilterColumn::Type => tab.filter_types.iter().cloned().map(FilterValue::Type).collect(),
        FilterColumn::Size => tab.filter_sizes.iter().copied().map(FilterValue::Size).collect(),
        FilterColumn::Date => tab.filter_dates.iter().copied().map(FilterValue::Date).collect(),
        FilterColumn::Deleted => tab.filter_deleted.iter().copied().map(FilterValue::Deleted).collect(),
        FilterColumn::Permissions => tab.filter_permissions.iter().copied().map(FilterValue::Permissions).collect(),
        FilterColumn::Name | FilterColumn::Location => Vec::new(),
    };
    for value in &checked {
        if seen.insert(value) {
            present.push(value.clone());
        }
    }
    present.sort_by_key(order_key);
    present
        .into_iter()
        .map(|value| FilterEntry {
            label: match &value {
                FilterValue::Type(label) => type_label(label, i18n),
                FilterValue::Size(category) => i18n.text(category.message()).to_string(),
                FilterValue::Date(group) | FilterValue::Deleted(group) => i18n.text(group.message()).to_string(),
                FilterValue::Permissions(mode) => crate::ui::format::permissions::paired(*mode),
            },
            count: counts.get(&value).copied().unwrap_or(0),
            sample: match &value {
                FilterValue::Type(label) => samples.get(label).cloned(),
                _ => None,
            },
            value,
        })
        .collect()
}

/// What the filter of `column` keeps, as a header's tooltip reads it; `None` when unset.
pub(crate) fn filter_summary(tab: &TabState, column: FilterColumn, i18n: &I18n, format: crate::ui::format::DateFormat) -> Option<String> {
    let language = i18n.language();
    let list = |mut labels: Vec<String>| {
        labels.sort();
        let more = labels.len().saturating_sub(3);
        labels.truncate(3);
        let text = labels.join(", ");
        if more > 0 { format!("{text} (+{more})") } else { text }
    };
    let bounds = |low: Option<String>, high: Option<String>| match (low, high) {
        (Some(low), Some(high)) => format!("{low} – {high}"),
        (Some(low), None) => format!("≥ {low}"),
        (None, Some(high)) => format!("≤ {high}"),
        (None, None) => String::new(),
    };
    let text = match column {
        FilterColumn::Name => tab.filter_name.clone(),
        FilterColumn::Location => tab.filter_original_location.clone(),
        FilterColumn::Type => list(tab.filter_types.iter().map(|label| type_label(label, i18n)).collect()),
        FilterColumn::Permissions => list(tab.filter_permissions.iter().map(|mode| crate::ui::format::permissions::paired(*mode)).collect()),
        FilterColumn::Size => match tab.filter_size_range {
            Some(range) => {
                let size = |bytes: Option<u64>| bytes.map(|bytes| crate::ui::format::format_size(bytes, false, false, language));
                bounds(size(range.min), size(range.max))
            }
            None => {
                let mut sizes: Vec<_> = tab.filter_sizes.iter().copied().collect();
                sizes.sort_by_key(|category| SizeCategory::ALL.iter().position(|c| c == category));
                sizes.into_iter().map(|category| i18n.text(category.message()).to_string()).collect::<Vec<_>>().join(", ")
            }
        },
        FilterColumn::Date | FilterColumn::Deleted => match if column == FilterColumn::Date { tab.filter_date_range } else { tab.filter_deleted_range } {
            Some(range) => {
                let day = |date: Option<chrono::NaiveDate>| date.map(|date| date.format(crate::ui::format::date_pattern(format)).to_string());
                bounds(day(range.from), day(range.to))
            }
            None => {
                let mut groups: Vec<_> = if column == FilterColumn::Date { &tab.filter_dates } else { &tab.filter_deleted }.iter().copied().collect();
                groups.sort();
                groups.into_iter().map(|group| i18n.text(group.message()).to_string()).collect::<Vec<_>>().join(", ")
            }
        },
    };
    (!text.is_empty()).then_some(text)
}

/// One line per set filter, its column named, for the status bar's tooltip.
pub(crate) fn filters_summary(tab: &TabState, i18n: &I18n, format: crate::ui::format::DateFormat) -> String {
    let trash = tab.path == "trash://";
    [
        (FilterColumn::Name, Message::FilterSummaryName),
        (FilterColumn::Location, if trash { Message::TooltipOriginalLocation } else { Message::TooltipLocation }),
        (FilterColumn::Type, Message::TooltipType),
        (FilterColumn::Size, Message::TooltipSize),
        (FilterColumn::Permissions, Message::TooltipPermissions),
        (FilterColumn::Date, Message::TooltipModified),
        (FilterColumn::Deleted, Message::TooltipDeleted),
    ]
    .into_iter()
    .filter_map(|(column, message)| {
        let value = filter_summary(tab, column, i18n, format)?;
        let mut args = crate::ui::i18n::FluentArgs::new();
        args.set("value", value);
        Some(i18n.text_with_args(message, &args).to_string())
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn value_of(file: &oxyde_lib::models::FileEntry, column: FilterColumn, today: chrono::NaiveDate) -> Option<FilterValue> {
    match column {
        FilterColumn::Type => Some(FilterValue::Type(file_type_label(file))),
        FilterColumn::Size => SizeCategory::ALL.into_iter().find(|category| category.matches(file.size)).map(FilterValue::Size),
        FilterColumn::Date => Some(FilterValue::Date(DateGroup::for_timestamp(file.modified, today))),
        FilterColumn::Deleted => crate::ui::workspace::deleted_ms(file).map(|ms| FilterValue::Deleted(DateGroup::for_timestamp(ms, today))),
        FilterColumn::Permissions => file.mode.map(FilterValue::Permissions),
        FilterColumn::Name | FilterColumn::Location => None,
    }
}

/// Kinds first, then extensions alphabetically; sizes and periods in their own order.
fn order_key(value: &FilterValue) -> (usize, String) {
    match value {
        FilterValue::Type(label) => match KINDS.iter().position(|kind| kind == label) {
            Some(index) => (index, String::new()),
            None => (KINDS.len(), label.clone()),
        },
        FilterValue::Size(category) => (SizeCategory::ALL.iter().position(|c| c == category).unwrap_or(0), String::new()),
        FilterValue::Date(group) | FilterValue::Deleted(group) => (DateGroup::ALL.iter().position(|g| g == group).unwrap_or(0), String::new()),
        FilterValue::Permissions(mode) => (*mode as usize, String::new()),
    }
}

#[cfg(test)]
#[path = "tests/filters.rs"]
mod tests;
