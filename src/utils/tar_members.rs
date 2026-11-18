//! TAR headers taken as members, shared by listing, decoding, opening and
//! extraction so every path sees the same entries as `tar_fidelity`.
use std::io::{self, Read};

/// Skips PAX global headers (`g`): they describe the archive, not a member
/// (`git archive` writes the commit there, so GitHub source archives start
/// with one). Members are numbered after this filter.
pub(crate) fn members<'a, R: Read>(
    entries: tar::Entries<'a, R>,
) -> impl Iterator<Item = io::Result<tar::Entry<'a, R>>> {
    entries.filter(|entry| {
        !entry
            .as_ref()
            .is_ok_and(|entry| entry.header().entry_type().is_pax_global_extensions())
    })
}

/// `Some(true)` for a directory, `Some(false)` for a file, `None` for links
/// and special files. Contiguous (`7`) and GNU sparse (`S`) members are
/// files: the tar crate expands sparse holes and reports the real size.
/// PAX sparse members (`GNU.sparse.*`) are refused, since the crate would
/// return their sparse map as content under a `GNUSparseFile.N` name.
pub(crate) fn directory<R: Read>(entry: &mut tar::Entry<'_, R>) -> io::Result<Option<bool>> {
    let kind = entry.header().entry_type();
    if let Some(extensions) = entry.pax_extensions()? {
        for extension in extensions {
            if extension?.key_bytes().starts_with(b"GNU.sparse.") {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "PAX sparse TAR members are not supported",
                ));
            }
        }
    }
    Ok(if kind.is_dir() {
        Some(true)
    } else if kind.is_file() || kind.is_contiguous() || kind.is_gnu_sparse() {
        Some(false)
    } else {
        None
    })
}

#[cfg(test)]
#[path = "tar_members/tests/members.rs"]
mod tests;
