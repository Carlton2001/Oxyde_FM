//! Undo and Redo of a move to the freedesktop trash. Undo restores the entries the deletion
//! created, once each is found to hold the object deleted: another item later trashed from
//! the same place is never taken for it. Redo trashes each restored object again, checked,
//! and keeps its new entry for the next Undo.
use crate::models::{
    error::HistoryDeleteFailure,
    transaction::{HistoryDeletedItem, HistoryFileIdentity},
    CommandError, TransactionDetails,
};
use crate::platform::platform;
use std::path::Path;

fn failure(path: &str, changed: bool) -> CommandError {
    CommandError::TrashError(HistoryDeleteFailure { changed, path: path.into() }.to_string())
}

/// The object at `path`, never followed if it is a link.
fn identity_at(path: &Path) -> Option<HistoryFileIdentity> {
    use rustix::fs::{Mode, OFlags};
    let item = rustix::fs::open(path, OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC, Mode::empty()).ok()?;
    HistoryFileIdentity::from_file(&std::fs::File::from(item)).ok()
}

/// The entries a deletion journaled, when it journaled each of them; older journals
/// name the original paths only.
pub(crate) fn journal(details: &TransactionDetails) -> Option<&[HistoryDeletedItem]> {
    let entries = details.deleted_items.as_deref().filter(|entries| !entries.is_empty())?;
    entries.iter().all(|entry| entry.identity.is_some()).then_some(entries)
}

pub(crate) async fn undo(entries: &[HistoryDeletedItem]) -> Result<(), CommandError> {
    // Every entry still in the trash must hold its object before any is restored.
    let mut contents = Vec::new();
    for entry in entries {
        let Some(content) = entry.content() else { continue };
        if identity_at(&content) != entry.identity {
            return Err(failure(&entry.path, true));
        }
        contents.push(crate::utils::path_text::encode(&content));
    }
    let result = crate::commands::ops::restore_items(contents).await;
    for entry in entries {
        if entry.content().is_some_and(|content| std::fs::symlink_metadata(content).is_err()) {
            entry.set_content(None);
        }
    }
    result.map(drop)
}

pub(crate) fn redo(entries: &[HistoryDeletedItem]) -> Result<(), CommandError> {
    let restored: Vec<_> = entries.iter().filter(|entry| entry.content().is_none()).collect();
    for entry in &restored {
        if identity_at(Path::new(&entry.path)) != entry.identity {
            return Err(failure(&entry.path, true));
        }
    }
    for entry in restored {
        let outcome = platform().shell_delete_tracked(Path::new(&entry.path), entry.identity);
        match (outcome.error, outcome.effects.first().and_then(HistoryDeletedItem::content)) {
            (None, Some(content)) if outcome.completed => entry.set_content(Some(content)),
            (error, _) => return Err(CommandError::TrashError(error.unwrap_or_else(|| "Missing completion receipt".into()))),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "linux_trash_history/tests/history.rs"]
mod tests;
