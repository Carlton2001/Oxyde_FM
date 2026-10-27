//! Folders a move renamed whole: Undo renames each back, Redo renames it again,
//! only when it is still the same folder and the other name is free.
use crate::models::error::{HistoryIdentityIssue, HistoryMoveFailure, HistoryMoveIssue, HistoryRenameFailure};
use crate::models::transaction::{HistoryRenamedDirectory, TransactionDetails};
use std::{io, path::Path};

/// Renames already applied, put back if the rest of the history action fails.
pub(crate) struct Renamed<'a> {
    done: Vec<&'a HistoryRenamedDirectory>,
    forward: bool,
}

fn rename(folder: &HistoryRenamedDirectory, forward: bool) -> io::Result<()> {
    let (from, to) = if forward { (&folder.source, &folder.path) } else { (&folder.path, &folder.source) };
    super::item_rename::rename_from_history(from, to, folder.identity).map_err(|error| as_move(error, from))
}

/// The move history explains a refusal, not the rename primitive it relies on.
fn as_move(error: io::Error, path: &Path) -> io::Error {
    let issue = match error.get_ref().and_then(|inner| inner.downcast_ref::<HistoryRenameFailure>()) {
        Some(failure) if failure.issue == HistoryIdentityIssue::Changed => HistoryMoveIssue::Changed,
        Some(_) => HistoryMoveIssue::Unverified,
        None if error.kind() == io::ErrorKind::AlreadyExists => HistoryMoveIssue::Conflict,
        None => return error,
    };
    io::Error::other(HistoryMoveFailure { issue, detail: path.to_string_lossy().into_owned() })
}

/// Undo renames back from the last; Redo renames again from the first.
pub(crate) fn apply(details: &TransactionDetails, forward: bool) -> io::Result<Renamed<'_>> {
    let folders = details.renamed_dirs.as_deref().unwrap_or_default();
    let mut renamed = Renamed { done: Vec::new(), forward };
    let ordered: Box<dyn Iterator<Item = &HistoryRenamedDirectory>> =
        if forward { Box::new(folders.iter()) } else { Box::new(folders.iter().rev()) };
    for folder in ordered {
        if let Err(error) = rename(folder, forward) {
            return Err(renamed.revert(error));
        }
        renamed.done.push(folder);
    }
    Ok(renamed)
}

impl Renamed<'_> {
    /// Puts every folder back under its previous name and returns `error`,
    /// naming any folder that could not be put back.
    pub(crate) fn revert(self, error: io::Error) -> io::Error {
        let kept: Vec<_> = self
            .done
            .into_iter()
            .rev()
            .filter_map(|folder| {
                rename(folder, !self.forward)
                    .err()
                    .map(|failure| format!("{}: {failure}", folder.path.display()))
            })
            .collect();
        if kept.is_empty() {
            error
        } else {
            io::Error::new(error.kind(), format!("{error}; folders left renamed: {}", kept.join("; ")))
        }
    }
}
