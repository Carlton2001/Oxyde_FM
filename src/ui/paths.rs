//! Paths compared as the platform treats them.

use std::path::{Path, PathBuf};

/// The form two spellings of one path share. Windows ignores separators, the
/// extended-length prefix, trailing separators and case, except inside a WSL
/// distribution, whose Linux names keep their case; elsewhere paths compare by
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
        let text = text.trim_end_matches('\\');
        if let Some(rest) = oxyde_lib::platform::wsl_relative(text) {
            // Both names of the root, and the distribution's name, ignore case.
            let rest = rest.trim_start_matches('\\');
            let (distribution, inner) = rest.split_once('\\').unwrap_or((rest, ""));
            return PathBuf::from(format!(r"{}\{}\{inner}", oxyde_lib::platform::WSL_ROOT, distribution.to_lowercase())
                .trim_end_matches('\\'));
        }
        PathBuf::from(text.to_lowercase())
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_path_buf()
    }
}

/// Whether two paths name the same place.
pub(crate) fn paths_match(left: &str, right: &str) -> bool {
    left == right || comparable(Path::new(left)) == comparable(Path::new(right))
}

/// Whether `path` is `root` or lies under it, by whole components: `folder-other`
/// is not under `folder`.
pub(crate) fn path_within(path: &Path, root: &Path) -> bool {
    comparable(path).starts_with(comparable(root))
}

#[cfg(test)]
#[path = "paths/tests/matching.rs"]
mod tests;
