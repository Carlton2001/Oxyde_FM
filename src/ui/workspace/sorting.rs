use super::{SortColumn, SortDirection};
use oxyde_lib::models::FileEntry;

// Lowercase once when preparing keys; compare digit runs without integer parsing
// so arbitrarily long numbers retain their numeric order.
#[derive(Eq, PartialEq)]
struct NaturalName(String);
impl Ord for NaturalName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let (mut left, mut right) = (self.0.as_str(), other.0.as_str());
        while !left.is_empty() && !right.is_empty() {
            let a = left.as_bytes()[0];
            let b = right.as_bytes()[0];
            if a.is_ascii_digit() && b.is_ascii_digit() {
                let alen = left.bytes().take_while(u8::is_ascii_digit).count();
                let blen = right.bytes().take_while(u8::is_ascii_digit).count();
                let order = oxyde_lib::utils::compare_natural(&left[..alen], &right[..blen]);
                if !order.is_eq() {
                    return order;
                }
                left = &left[alen..];
                right = &right[blen..];
            } else {
                let ac = left.chars().next().unwrap();
                let bc = right.chars().next().unwrap();
                let order = ac.cmp(&bc);
                if !order.is_eq() {
                    return order;
                }
                left = &left[ac.len_utf8()..];
                right = &right[bc.len_utf8()..];
            }
        }
        left.len().cmp(&right.len())
    }
}
impl PartialOrd for NaturalName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

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
        let name = NaturalName(file.name.to_lowercase());
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

#[cfg(test)]
#[path = "sorting/tests/natural.rs"]
mod tests;
