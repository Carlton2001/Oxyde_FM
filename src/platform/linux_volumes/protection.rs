//! Which mounts refuse writes, from the kernel mount table, and the links by which a
//! read-only drive leads elsewhere: the drive list hides `/var`, `/home` and other
//! writable mounts, so the listed `/` alone would decide for the whole system.
use super::mounts::MountEntry;
use crate::models::DriveInfo;
use crate::platform::WriteProtection;
use std::path::{Component, Path, PathBuf};

/// Every mount comes from the table, without touching a network share; only the top of a
/// local read-only drive is read, for its links.
pub(super) fn write_protection(table: &[MountEntry], roots: &[DriveInfo]) -> WriteProtection {
    let mut mounts: Vec<(String, bool)> = Vec::new();
    for entry in table {
        // A later mount on the same point hides the earlier one.
        match mounts.iter_mut().find(|(point, _)| *point == entry.point) {
            Some(mount) => mount.1 = entry.read_only,
            None => mounts.push((entry.point.clone(), entry.read_only)),
        }
    }
    let links = roots
        .iter()
        .filter(|root| root.drive_type != "remote")
        .filter(|root| mounts.iter().any(|(point, read_only)| *read_only && *point == root.path))
        .flat_map(|root| top_links(Path::new(&root.path)))
        .collect();
    WriteProtection { mounts, links }
}

/// Links directly under `root`, with their target resolved lexically: the target is never
/// opened, so a link to an unreachable share cannot block.
fn top_links(root: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_symlink()))
        .filter_map(|entry| {
            let link = entry.path();
            let target = lexical(&root.join(std::fs::read_link(&link).ok()?))?;
            Some((link.to_string_lossy().into_owned(), target.to_string_lossy().into_owned()))
        })
        .collect()
}

/// `.` and `..` removed without following links; `..` stops at `/`, as the kernel does.
fn lexical(path: &Path) -> Option<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            other => resolved.push(other),
        }
    }
    resolved.is_absolute().then_some(resolved)
}

#[cfg(test)]
#[path = "tests/protection.rs"]
mod tests;
