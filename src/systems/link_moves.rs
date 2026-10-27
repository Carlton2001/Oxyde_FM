//! Move one symbolic link between held parents, for a Move and its Undo/Redo.
use crate::platform::platform;
use std::{fs::File, io, path::Path};

/// Rename the link on one volume; across volumes, recreate it then remove the
/// source. `verify` accepts only the expected source link, before and after.
pub(crate) fn move_link(
    source_parent: &File,
    source: &Path,
    destination_parent: &File,
    destination: &Path,
    target: &Path,
    replace: bool,
    verify: &mut dyn FnMut(&File) -> io::Result<()>,
) -> io::Result<File> {
    verify(&platform().track_source_item(source_parent, source)?)?;
    match platform().rename_link(source_parent, source, destination_parent, destination, replace) {
        Ok(moved) => {
            // A link substituted after the check is put back under its own name.
            if let Err(error) = verify(&moved) {
                if let Err(restore) =
                    platform().rename_link(destination_parent, destination, source_parent, source, false)
                {
                    return Err(io::Error::new(
                        error.kind(),
                        format!("{error}; link left at {}: {restore}", destination.display()),
                    ));
                }
                return Err(error);
            }
            Ok(moved)
        }
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            let created = platform().create_link(destination_parent, destination, target, replace)?;
            if let Err(error) = platform().remove_link(source_parent, source, verify) {
                // Undo the new link unless it already replaced the destination.
                let undone = if replace {
                    Err(io::Error::other("it replaced the destination"))
                } else {
                    let identity = crate::models::transaction::HistoryFileIdentity::from_file(&created)?;
                    platform().remove_link(destination_parent, destination, &mut |current| {
                        if identity.matches(current)? {
                            Ok(())
                        } else {
                            Err(io::Error::other("Destination link changed"))
                        }
                    })
                };
                return Err(match undone {
                    Ok(()) => error,
                    Err(kept) => io::Error::new(
                        error.kind(),
                        format!("{error}; copy kept at {}: {kept}", destination.display()),
                    ),
                });
            }
            Ok(created)
        }
        Err(error) => Err(error),
    }
}
