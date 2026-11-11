//! Paths compared as the platform treats them.

use std::path::{Path, PathBuf};

/// The form two spellings of one path share. Windows ignores separators, the
/// extended-length prefix, trailing separators and case; elsewhere paths compare by
/// component, so only a trailing separator differs.
fn comparable(path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let mut text = path.to_string_lossy().replace('/', "\\");
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            text = format!(r"\\{rest}");
        } else if let Some(rest) = text.strip_prefix(r"\\?\") {
            text = rest.to_string();
        }
        PathBuf::from(text.trim_end_matches('\\').to_lowercase())
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_path_buf()
    }
}

/// Whether two paths name the same place.
pub(crate) fn paths_match(left: &str, right: &str) -> bool {
    comparable(Path::new(left)) == comparable(Path::new(right))
}

/// Whether `path` is `root` or lies under it, by whole components: `folder-other`
/// is not under `folder`.
pub(crate) fn path_within(path: &Path, root: &Path) -> bool {
    comparable(path).starts_with(comparable(root))
}

#[cfg(test)]
#[path = "paths/tests/matching.rs"]
mod tests;
