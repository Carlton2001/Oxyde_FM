use crate::ui::components::icons::{entry_icon, IconType};
use oxyde_lib::models::FileEntry;

#[cfg(test)]
#[path = "tests/icons.rs"]
mod tests;

/// The item on disk: at its source, or where it went once moved. Blocking: the
/// list loads its items in the background.
pub(super) fn resolve_entry(
    source: &std::path::Path,
    destination: Option<&std::path::Path>,
) -> Result<FileEntry, oxyde_lib::models::CommandError> {
    oxyde_lib::models::get_file_entry_from_path(source).or_else(|error| match destination {
        Some(destination) => oxyde_lib::models::get_file_entry_from_path(destination),
        None => Err(error),
    })
}

#[cfg(test)]
fn resolve(
    source: &std::path::Path,
    destination: Option<&std::path::Path>,
) -> Result<(String, IconType), oxyde_lib::models::CommandError> {
    resolve_entry(source, destination).map(|entry| (entry.path.clone(), entry_icon(&entry)))
}

/// The icon a row shows: its entry's, or the name's when the item is gone.
pub(super) fn icon(entry: Option<&FileEntry>, path: &std::path::Path) -> (String, IconType) {
    match entry {
        Some(entry) => (entry.path.clone(), entry_icon(entry)),
        None => {
            let full = path.to_string_lossy().into_owned();
            let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned();
            let icon = crate::ui::components::icons::get_icon_type(&name, false, Some(&full), false, false);
            (full, icon)
        }
    }
}
