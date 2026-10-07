use crate::ui::i18n::Message;
use chrono::{Datelike, Duration, Local, NaiveDate};
use oxyde_lib::models::FileEntry;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DateGroup {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    Older,
}

impl DateGroup {
    pub const ALL: [Self; 6] = [
        Self::Today,
        Self::Yesterday,
        Self::ThisWeek,
        Self::LastWeek,
        Self::ThisMonth,
        Self::Older,
    ];
    pub fn message(self) -> Message {
        match self {
            Self::Today => Message::DateToday,
            Self::Yesterday => Message::DateYesterday,
            Self::ThisWeek => Message::DateThisWeek,
            Self::LastWeek => Message::DateLastWeek,
            Self::ThisMonth => Message::DateThisMonth,
            Self::Older => Message::DateOlder,
        }
    }
    pub fn for_date(date: NaiveDate, today: NaiveDate) -> Self {
        let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        if date >= today {
            Self::Today
        } else if date >= today - Duration::days(1) {
            Self::Yesterday
        } else if date >= monday {
            Self::ThisWeek
        } else if date >= monday - Duration::days(7) {
            Self::LastWeek
        } else if date >= today.with_day(1).unwrap() {
            Self::ThisMonth
        } else {
            Self::Older
        }
    }
    pub fn for_timestamp(ms: u64, today: NaiveDate) -> Self {
        Self::for_timestamp_in(ms, today, &Local)
    }

    fn for_timestamp_in<T: chrono::TimeZone>(ms: u64, today: NaiveDate, timezone: &T) -> Self {
        i64::try_from(ms)
            .ok()
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(|date| Self::for_date(date.with_timezone(timezone).date_naive(), today))
            .unwrap_or(Self::Older)
    }
}

/// Counts the same exclusive calendar periods used for grouping and filtering.
pub fn counts_by_date(files: &[FileEntry], today: NaiveDate) -> HashMap<DateGroup, u64> {
    let mut counts = HashMap::new();
    for file in files {
        *counts
            .entry(DateGroup::for_timestamp(file.modified, today))
            .or_default() += 1;
    }
    counts
}

#[derive(Clone, Debug)]
pub enum DisplayRow {
    Header(DateGroup, usize),
    Files(Vec<usize>),
}

pub fn row_height(row: &DisplayRow, grid: bool) -> f32 {
    if grid && matches!(row, DisplayRow::Files(_)) {
        crate::ui::style::grid_card_row_height()
    } else {
        crate::ui::style::row_height()
    }
}

pub fn content_height(rows: &[DisplayRow], grid: bool) -> f32 {
    rows.iter().map(|row| row_height(row, grid)).sum()
}

pub fn visible_indices(rows: &[DisplayRow]) -> Vec<usize> {
    rows.iter()
        .filter_map(|row| match row {
            DisplayRow::Files(indices) => Some(indices.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

/// Keeps the current sort inside each category. Indices refer to the filtered cache.
pub fn display_rows(
    files: &[FileEntry],
    enabled: bool,
    collapsed: &HashSet<DateGroup>,
    columns: usize,
) -> Vec<DisplayRow> {
    let columns = columns.max(1);
    if !enabled {
        return (0..files.len())
            .collect::<Vec<_>>()
            .chunks(columns)
            .map(|indices| DisplayRow::Files(indices.to_vec()))
            .collect();
    }
    let today = Local::now().date_naive();
    let mut groups: [Vec<usize>; 6] = Default::default();
    for (index, file) in files.iter().enumerate() {
        let group = DateGroup::for_timestamp(file.modified, today);
        groups[DateGroup::ALL
            .iter()
            .position(|candidate| *candidate == group)
            .unwrap()]
        .push(index);
    }
    let mut rows = Vec::new();
    for (group, indices) in DateGroup::ALL.into_iter().zip(groups) {
        if indices.is_empty() {
            continue;
        }
        rows.push(DisplayRow::Header(group, indices.len()));
        if !collapsed.contains(&group) {
            rows.extend(
                indices
                    .chunks(columns)
                    .map(|chunk| DisplayRow::Files(chunk.to_vec())),
            );
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }
    #[test]
    fn calendar_boundaries_match_tauri() {
        let today = date(2026, 10, 3);
        for (day, group) in [
            (3, DateGroup::Today),
            (2, DateGroup::Yesterday),
            (1, DateGroup::ThisWeek),
        ] {
            assert_eq!(DateGroup::for_date(date(2026, 10, day), today), group);
        }
        assert_eq!(
            DateGroup::for_date(date(2026, 9, 28), today),
            DateGroup::ThisWeek
        );
        assert_eq!(
            DateGroup::for_date(date(2026, 9, 21), today),
            DateGroup::LastWeek
        );
        assert_eq!(
            DateGroup::for_date(date(2026, 9, 20), today),
            DateGroup::Older
        );
        assert_eq!(
            DateGroup::for_date(date(2026, 9, 1), date(2026, 9, 30)),
            DateGroup::ThisMonth
        );
        assert_eq!(
            DateGroup::for_date(date(2026, 9, 27), date(2026, 9, 28)),
            DateGroup::Yesterday
        );
        assert_eq!(DateGroup::for_timestamp(u64::MAX, today), DateGroup::Older);
    }

    #[test]
    fn calendar_groups_keep_priority_across_month_and_year_boundaries() {
        for (today, modified, expected) in [
            (date(2027, 1, 1), date(2026, 12, 31), DateGroup::Yesterday),
            (date(2027, 1, 1), date(2026, 12, 28), DateGroup::ThisWeek),
            (date(2027, 1, 1), date(2026, 12, 27), DateGroup::LastWeek),
            (date(2027, 1, 1), date(2026, 12, 20), DateGroup::Older),
            (date(2026, 10, 1), date(2026, 9, 30), DateGroup::Yesterday),
            (date(2026, 10, 1), date(2026, 9, 28), DateGroup::ThisWeek),
            (date(2026, 10, 28), date(2026, 10, 18), DateGroup::ThisMonth),
            (date(2026, 10, 28), date(2026, 10, 29), DateGroup::Today),
        ] {
            assert_eq!(DateGroup::for_date(modified, today), expected);
        }
    }

    #[test]
    fn local_calendar_day_wins_over_elapsed_hours_with_offset_changes() {
        use chrono::{FixedOffset, TimeZone};
        let winter = FixedOffset::east_opt(3600).unwrap();
        let summer = FixedOffset::east_opt(7200).unwrap();
        // A 23-hour day: yesterday is still yesterday before 24 hours elapse.
        let now = summer
            .with_ymd_and_hms(2026, 3, 30, 0, 1, 0)
            .single()
            .unwrap();
        let modified = winter
            .with_ymd_and_hms(2026, 3, 29, 0, 5, 0)
            .single()
            .unwrap();
        assert!(now.signed_duration_since(modified).num_hours() < 24);
        assert_eq!(
            DateGroup::for_timestamp_in(
                modified.timestamp_millis() as u64,
                now.date_naive(),
                &winter
            ),
            DateGroup::Yesterday
        );
        // A 25-hour day: today stays today after 24 hours have elapsed.
        let now = winter
            .with_ymd_and_hms(2026, 10, 25, 23, 30, 0)
            .single()
            .unwrap();
        let modified = summer
            .with_ymd_and_hms(2026, 10, 25, 0, 5, 0)
            .single()
            .unwrap();
        assert!(now.signed_duration_since(modified).num_minutes() > 24 * 60);
        assert_eq!(
            DateGroup::for_timestamp_in(
                modified.timestamp_millis() as u64,
                now.date_naive(),
                &summer
            ),
            DateGroup::Today
        );
    }

    #[test]
    fn timestamp_conversion_uses_the_local_date_and_rejects_invalid_values() {
        use chrono::{FixedOffset, TimeZone};
        let timezone = FixedOffset::east_opt(7200).unwrap();
        let modified = chrono::Utc
            .with_ymd_and_hms(2026, 10, 6, 22, 30, 0)
            .single()
            .unwrap();
        let today = date(2026, 10, 7);
        assert_eq!(
            DateGroup::for_timestamp_in(modified.timestamp_millis() as u64, today, &timezone),
            DateGroup::Today
        );
        assert_eq!(
            DateGroup::for_timestamp_in(u64::MAX, today, &timezone),
            DateGroup::Older
        );
        assert_eq!(
            DateGroup::for_timestamp_in(i64::MAX as u64, today, &timezone),
            DateGroup::Older
        );
        assert_eq!(
            DateGroup::for_timestamp_in(0, today, &timezone),
            DateGroup::Older
        );
        assert!(counts_by_date(&[], today).is_empty());
    }

    #[test]
    fn rows_keep_sort_order_separate_grid_groups_and_skip_collapsed_entries() {
        let now = Local::now().timestamp_millis() as u64;
        let files = [0, now, now, 0, now].map(|modified| FileEntry {
            modified,
            ..FileEntry::default()
        });
        let rows = display_rows(&files, true, &HashSet::new(), 2);
        assert!(matches!(rows[0], DisplayRow::Header(DateGroup::Today, 3)));
        assert!(matches!(&rows[1], DisplayRow::Files(indices) if indices == &[1, 2]));
        assert!(matches!(&rows[2], DisplayRow::Files(indices) if indices == &[4]));
        assert!(matches!(rows[3], DisplayRow::Header(DateGroup::Older, 2)));
        assert_eq!(visible_indices(&rows), vec![1, 2, 4, 0, 3]);
        let collapsed = HashSet::from([DateGroup::Today]);
        let rows = display_rows(&files, true, &collapsed, 2);
        assert!(matches!(rows[0], DisplayRow::Header(DateGroup::Today, 3)));
        assert_eq!(visible_indices(&rows), vec![0, 3]);
        assert_eq!(
            visible_indices(&display_rows(&files, false, &collapsed, 2)),
            vec![0, 1, 2, 3, 4]
        );
        assert!(display_rows(&[], true, &collapsed, 1).is_empty());
    }
}
