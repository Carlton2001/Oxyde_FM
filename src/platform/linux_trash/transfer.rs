//! Items leaving the trash by Cut and Paste or a drag between panes: the transfer engine
//! gives them their original name, then detaches the record of each one that left, keeping
//! it for Undo. Only a listed item qualifies, not what lies inside a trashed folder.
use super::{content_path, forget_directory_size_of, trash_root};
use crate::models::transaction::{HistoryFileIdentity, HistoryMoveIdentity};
use crate::platform::{platform, DetachedTrashMetadata, TrashTransferEntry};
use crate::systems::path_parents::HeldParents;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Records hold one path; anything larger is not one.
const MAX_RECORD_BYTES: u64 = 128 * 1024;

fn record_bytes(file: &mut File) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(io::Error::other("Trash record is too large"));
    }
    Ok(bytes)
}

pub(in crate::platform) fn entries(paths: &[PathBuf]) -> Result<Vec<TrashTransferEntry>, String> {
    // Most transfers never touch the trash: avoid listing it.
    let candidates: Vec<_> = paths.iter().filter(|path| super::inside_trash(path)).collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let items = platform().list_trash_items()?;
    candidates.into_iter().map(|path| {
        let item = items.iter().find(|item| content_path(item).as_deref() == Some(path))
            .ok_or_else(|| format!("Only whole items can leave the trash: {}", path.display()))?;
        entry(path, item).map_err(|error| error.to_string())
    }).collect()
}

fn entry(path: &Path, item: &trash::TrashItem) -> io::Result<TrashTransferEntry> {
    let metadata_path = PathBuf::from(&item.id);
    let mut file = platform().open_archive_source(&metadata_path, false)?;
    let bytes = record_bytes(&mut file)?;
    Ok(TrashTransferEntry {
        path: path.to_path_buf(),
        original_path: item.original_path(),
        metadata_identity: HistoryFileIdentity::from_file(&file)?,
        metadata_hash: blake3::hash(&bytes).to_hex().to_string(),
        metadata_path,
    })
}

/// Moves the record of an item that left aside, under a name no trash listing reads. An item
/// still there, skipped or cancelled, keeps its record, as does one whose record changed.
pub(in crate::platform) fn detach(entry: &TrashTransferEntry) -> Result<Option<DetachedTrashMetadata>, String> {
    detach_checked(entry).map_err(|error| error.to_string())
}

fn detach_checked(entry: &TrashTransferEntry) -> io::Result<Option<DetachedTrashMetadata>> {
    match std::fs::symlink_metadata(&entry.path) {
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parents = HeldParents::new(&entry.metadata_path)?;
    let source = parents.resolved_path(&entry.metadata_path)?;
    let mut file = platform().open_copied_file(parents.directory(), &source)?;
    if !entry.metadata_identity.matches(&file)? {
        return Err(io::Error::other("Trash record was replaced; preserved"));
    }
    let hash = blake3::hash(&record_bytes(&mut file)?).to_hex().to_string();
    if hash != entry.metadata_hash {
        return Err(io::Error::other(format!("Trash record changed during transfer: {}", entry.path.display())));
    }
    // Capture before mutation: a record without a usable proof stays where it was.
    let identity = HistoryMoveIdentity::from_file(&file)?;
    let root = trash_root(&entry.metadata_path).ok_or_else(|| io::Error::other("Invalid trash record"))?;
    let backup = root.join(format!(".oxyde-trash-{}.tmp", uuid::Uuid::new_v4()));
    let backup_parents = HeldParents::new(&backup)?;
    platform().rename_operation_file(&file, &source, backup_parents.directory(), &backup_parents.resolved_path(&backup)?, false)?;
    forget_directory_size_of(&entry.metadata_path);
    Ok(Some(DetachedTrashMetadata { source: entry.metadata_path.clone(), backup, identity, hash }))
}

#[cfg(test)]
#[path = "tests/transfer.rs"]
mod tests;
