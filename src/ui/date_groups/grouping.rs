use super::{DateGroup, DisplayRow, FileFamily};
use crate::ui::i18n::{I18n, Message};
use chrono::NaiveDate;
use gpui::SharedString;
use oxyde_lib::models::FileEntry;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupMode {
    #[default]
    None,
    Date,
    Type,
    Size,
    Extension,
    Created,
    Origin,
    Deleted,
}
impl GroupMode {
    pub const ALL: [Self; 8] = [
        Self::None,
        Self::Date,
        Self::Created,
        Self::Deleted,
        Self::Type,
        Self::Extension,
        Self::Size,
        Self::Origin,
    ];
    pub fn message(self) -> Message {
        match self {
            Self::None => Message::GroupNone,
            Self::Date => Message::GroupDate,
            Self::Type => Message::GroupType,
            Self::Size => Message::GroupSize,
            Self::Extension => Message::GroupExtension,
            Self::Created => Message::GroupCreated,
            Self::Origin => Message::TrashOriginalLocation,
            Self::Deleted => Message::GroupDeleted,
        }
    }
    pub fn is_calendar(self) -> bool {
        matches!(self, Self::Date | Self::Created | Self::Deleted)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SizeGroup {
    Empty,
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
}
impl SizeGroup {
    fn for_size(size: u64) -> Self {
        match size {
            0 => Self::Empty,
            1..10240 => Self::Tiny,
            10240..1048576 => Self::Small,
            1048576..104857600 => Self::Medium,
            104857600..1073741824 => Self::Large,
            _ => Self::Huge,
        }
    }
    fn message(self) -> Message {
        match self {
            Self::Empty => Message::GroupSizeEmpty,
            Self::Tiny => Message::SizeTiny,
            Self::Small => Message::SizeSmall,
            Self::Medium => Message::SizeMedium,
            Self::Large => Message::SizeLarge,
            Self::Huge => Message::SizeHuge,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GroupKey {
    Date(DateGroup),
    Family(FileFamily),
    Size(SizeGroup),
    Extension(String),
    NoExtension,
    Origin(String),
    MissingOrigin,
    MissingDate,
}
impl GroupKey {
    pub fn label(&self, i18n: &I18n) -> SharedString {
        match self {
            Self::Extension(extension) => extension.to_uppercase().into(),
            Self::Origin(path) => path.clone().into(),
            Self::Date(date) => i18n.text(date.message()),
            Self::Family(family) => i18n.text(family.message()),
            Self::Size(size) => i18n.text(size.message()),
            Self::NoExtension => i18n.text(Message::GroupNoExtension),
            Self::MissingOrigin => i18n.text(Message::GroupOriginUnknown),
            Self::MissingDate => i18n.text(Message::GroupDateUnknown),
        }
    }
}

fn calendar_group(timestamp: Option<u64>, today: NaiveDate) -> GroupKey {
    timestamp
        .filter(|timestamp| {
            i64::try_from(*timestamp)
                .ok()
                .and_then(chrono::DateTime::from_timestamp_millis)
                .is_some()
        })
        .map(|timestamp| GroupKey::Date(DateGroup::for_timestamp(timestamp, today)))
        .unwrap_or(GroupKey::MissingDate)
}

pub fn group_for_file(file: &FileEntry, mode: GroupMode, today: NaiveDate) -> GroupKey {
    match mode {
        GroupMode::None | GroupMode::Date => {
            GroupKey::Date(DateGroup::for_timestamp(file.modified, today))
        }
        GroupMode::Created => calendar_group(file.created, today),
        GroupMode::Deleted => calendar_group(
            file.deleted_time.and_then(|time| u64::try_from(time).ok()),
            today,
        ),
        GroupMode::Type => GroupKey::Family(FileFamily::for_file(file)),
        GroupMode::Size if file.is_dir && !file.is_calculated => {
            GroupKey::Family(FileFamily::Folders)
        }
        GroupMode::Size => GroupKey::Size(SizeGroup::for_size(file.size)),
        GroupMode::Extension if file.is_dir => GroupKey::Family(FileFamily::Folders),
        GroupMode::Extension => std::path::Path::new(&file.name)
            .extension()
            .and_then(|extension| extension.to_str())
            .filter(|extension| !extension.is_empty())
            .map(|extension| GroupKey::Extension(extension.to_ascii_lowercase()))
            .unwrap_or(GroupKey::NoExtension),
        GroupMode::Origin => {
            let path = file.original_path.as_deref().unwrap_or(&file.path);
            std::path::Path::new(path)
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(|parent| GroupKey::Origin(parent.to_string_lossy().into_owned()))
                .unwrap_or(GroupKey::MissingOrigin)
        }
    }
}

/// Preserve the current sort inside each section. Dynamic keys are ordered once,
/// rather than searching every existing category for each file.
pub fn display_rows(
    files: &[FileEntry],
    mode: GroupMode,
    collapsed: &HashSet<GroupKey>,
    columns: usize,
) -> Vec<DisplayRow> {
    display_data(files, mode, collapsed, columns).0
}

pub(crate) fn display_data(
    files: &[FileEntry],
    mode: GroupMode,
    collapsed: &HashSet<GroupKey>,
    columns: usize,
) -> (Vec<DisplayRow>, BTreeMap<GroupKey, Vec<usize>>) {
    let columns = columns.max(1);
    if mode == GroupMode::None {
        let rows = (0..files.len())
            .collect::<Vec<_>>()
            .chunks(columns)
            .map(|indices| DisplayRow::Files(indices.to_vec()))
            .collect();
        return (rows, BTreeMap::new());
    }
    let today = chrono::Local::now().date_naive();
    let mut groups: BTreeMap<GroupKey, Vec<usize>> = BTreeMap::new();
    for (index, file) in files.iter().enumerate() {
        groups
            .entry(group_for_file(file, mode, today))
            .or_default()
            .push(index);
    }
    let mut rows = Vec::new();
    for (group, indices) in &groups {
        rows.push(DisplayRow::Header(group.clone(), indices.len()));
        if !collapsed.contains(group) {
            rows.extend(
                indices
                    .chunks(columns)
                    .map(|indices| DisplayRow::Files(indices.to_vec())),
            );
        }
    }
    (rows, groups)
}

#[cfg(test)]
#[path = "tests/grouping.rs"]
mod tests;
