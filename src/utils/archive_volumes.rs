//! Volumes of a split RAR, found by their names as WinRAR writes them: `name.part1.rar`,
//! `name.part2.rar`… or `name.rar`, `name.r00`, `name.r01`… A set is the consecutive
//! volumes that exist; a missing one ends it, and decoding then reports the cut.
use super::archive_io_protocol::MAX_VOLUMES;
use std::path::{Path, PathBuf};

/// Every volume of the set `path` belongs to, the first one first; just `path` when it
/// is a single archive, or a later volume whose first one is missing.
pub fn rar_volumes(path: &Path) -> Vec<PathBuf> {
    let found = names(path).map(|name| {
        (0..MAX_VOLUMES).map_while(|index| name(index).filter(|path| path.is_file())).collect::<Vec<_>>()
    });
    match found {
        Some(set) if set.len() > 1 && set.iter().any(|volume| volume == path) => set,
        _ => vec![path.to_path_buf()],
    }
}

/// True when `path` is one volume of a set of several.
pub fn is_rar_volume_set(path: &Path) -> bool {
    rar_volumes(path).len() > 1
}

/// The first volume `path` names, by its name alone (`film.part3.rar` → `film.part1.rar`),
/// and the set's name without volume or extension (`film`). Touches no file.
pub fn rar_set_name(path: &Path) -> Option<(PathBuf, String)> {
    let name = path.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    let stem = &name[..lower.strip_suffix(".rar")?.len()];
    let dot = stem.to_ascii_lowercase().rfind(".part")?;
    let digits = &stem[dot + 5..];
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let first = format!("{}{:0width$}{}", &name[..dot + 5], 1, &name[stem.len()..], width = digits.len());
    Some((path.with_file_name(first), stem[..dot].to_owned()))
}

/// The name of the set's volume at an index, from 0, following the naming of `path`.
fn names(path: &Path) -> Option<Box<dyn Fn(usize) -> Option<PathBuf>>> {
    let name = path.file_name()?.to_str()?.to_owned();
    let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let stem_length = lower.strip_suffix(".rar")?.len();
    let stem = &lower[..stem_length];
    if let Some(dot) = stem.rfind(".part") {
        let digits = &stem[dot + 5..];
        if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
            let (prefix, extension) = (name[..dot + 5].to_owned(), name[stem_length..].to_owned());
            let width = digits.len();
            return Some(Box::new(move |index| {
                let number = format!("{:0width$}", index + 1);
                (number.len() == width).then(|| folder.join(format!("{prefix}{number}{extension}")))
            }));
        }
    }
    // The old naming: the `.rar` first, then `.r00` to `.r99`.
    let (stem, upper) = (name[..stem_length].to_owned(), name.ends_with('R'));
    Some(Box::new(move |index| match index {
        0 => Some(folder.join(&name)),
        1..=100 => {
            let letter = if upper { 'R' } else { 'r' };
            Some(folder.join(format!("{stem}.{letter}{:02}", index - 1)))
        }
        _ => None,
    }))
}

#[cfg(test)]
#[path = "archive_volumes/tests/volume_tests.rs"]
mod tests;
