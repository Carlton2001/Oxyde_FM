use super::{SortColumn, SortDirection};
use oxyde_lib::models::FileEntry;

pub(super) fn sort_files(files: &mut [FileEntry], col: SortColumn, direction: SortDirection) {
    sort(files, col, direction, false);
}

pub(super) fn sort_listed_files(
    files: &mut [FileEntry],
    col: SortColumn,
    direction: SortDirection,
) {
    sort(files, col, direction, true);
}

// The old listing applied a natural sort before the stable UI sort. Preserve
// that order only for UI-key ties, including Unicode names with equal lowercase.
struct ListingTie(Option<String>);
impl Ord for ListingTie {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => oxyde_lib::utils::compare_natural(left, right),
            _ => self.0.is_some().cmp(&other.0.is_some()),
        }
    }
}
impl PartialOrd for ListingTie {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for ListingTie {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for ListingTie {}

fn sort(files: &mut [FileEntry], col: SortColumn, direction: SortDirection, listed: bool) {
    // Build each normalized key once; keep folders first in both directions.
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum Value {
        Text(String),
        Number(u64),
        Deleted(Option<i64>),
        Location(Option<std::path::PathBuf>),
    }

    let key = |file: &FileEntry| {
        let name = file.name.to_lowercase();
        let value = match col {
            SortColumn::Name => Value::Text(String::new()),
            SortColumn::Type => Value::Text(
                std::path::Path::new(&file.name)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .unwrap_or("")
                    .to_lowercase(),
            ),
            SortColumn::Size => Value::Number(file.size),
            SortColumn::Modified => Value::Number(file.modified),
            SortColumn::DeletedAt => Value::Deleted(file.deleted_time),
            SortColumn::OriginalLocation => Value::Text(
                file.original_path
                    .as_deref()
                    .and_then(|path| std::path::Path::new(path).parent())
                    .map(|path| path.to_string_lossy().to_lowercase())
                    .unwrap_or_default(),
            ),
            SortColumn::Location => Value::Location(
                std::path::Path::new(&file.path)
                    .parent()
                    .map(std::path::Path::to_path_buf),
            ),
        };
        (value, name)
    };
    match direction {
        SortDirection::Ascending => files.sort_by_cached_key(|file| {
            (
                !file.is_dir,
                key(file),
                ListingTie(listed.then(|| file.name.clone())),
            )
        }),
        SortDirection::Descending => files.sort_by_cached_key(|file| {
            (
                !file.is_dir,
                std::cmp::Reverse(key(file)),
                ListingTie(listed.then(|| file.name.clone())),
            )
        }),
    }
}
