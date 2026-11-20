//! Redo of links a copy created: recreated only from the same source link,
//! with the same target, at a vacant destination.
use super::path_parents::HeldParents;
use crate::models::{error::SourceSelectionFailure, transaction::HistoryLink};
use crate::platform::platform;
use std::io;

fn verify_source(link: &HistoryLink) -> io::Result<()> {
    let parents = HeldParents::new(&link.source)?;
    let source = platform()
        .track_source_item(parents.directory(), &link.source)
        .map_err(|_| SourceSelectionFailure::changed(&link.source))?;
    let metadata = source.metadata()?;
    match platform().source_link_target(&source, &metadata) {
        Some(Ok(target)) if target == link.target => Ok(()),
        Some(Err(error)) => Err(error),
        _ => Err(SourceSelectionFailure::changed(&link.source)),
    }
}

/// Recreate every link, or none: a failure removes the links already recreated.
pub(crate) fn redo(links: &[HistoryLink]) -> io::Result<()> {
    for link in links {
        verify_source(link)?;
        if link.path.symlink_metadata().is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Cannot redo copy; destination already exists: {}", link.path.display()),
            ));
        }
    }
    for (index, link) in links.iter().enumerate() {
        let result = HeldParents::new(&link.path).and_then(|parents| {
            let created =
                platform().create_link(parents.directory(), &link.path, &link.target, false)?;
            link.identity.refresh_from_file(&created).map_err(|error| {
                // The new link has no history proof: remove this exact link now,
                // or Redo would next find its own leftover in the way.
                match remove_created(&parents, link, &created) {
                    Ok(()) => error,
                    Err(rollback) => io::Error::new(
                        error.kind(),
                        format!("{error}; new link left in place: {rollback}"),
                    ),
                }
            })
        });
        if let Err(error) = result {
            return Err(match remove(&links[..index]) {
                Ok(()) => error,
                Err(rollback) => io::Error::new(
                    error.kind(),
                    format!("{error}; rollback incomplete: {rollback}"),
                ),
            });
        }
    }
    Ok(())
}

fn remove_created(parents: &HeldParents, link: &HistoryLink, created: &std::fs::File) -> io::Result<()> {
    let expected = platform().file_identity(created)?;
    platform().remove_link(parents.directory(), &link.path, &mut |current| {
        if platform().file_identity(current)? == expected {
            Ok(())
        } else {
            Err(super::file_copy::identity_changed(&link.path))
        }
    })
}

/// Remove recreated links that are still exactly the links Redo created.
pub(crate) fn remove(links: &[HistoryLink]) -> io::Result<()> {
    let mut errors = Vec::new();
    for link in links.iter().rev() {
        let result = HeldParents::new(&link.path).and_then(|parents| {
            platform().remove_link(parents.directory(), &link.path, &mut |current| {
                let identity = link
                    .identity
                    .snapshot()
                    .ok_or_else(|| super::file_copy::identity_changed(&link.path))?;
                if identity.matches(current)? {
                    Ok(())
                } else {
                    Err(super::file_copy::identity_changed(&link.path))
                }
            })
        });
        if let Err(error) = result {
            errors.push(format!("{}: {error}", link.path.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(errors.join("; ")))
    }
}
