//! Archive names are untrusted data, not native filesystem paths.
use crate::platform::platform;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

pub fn relative_archive_path(name: &str) -> io::Result<PathBuf> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err(unsafe_name(name));
    }
    let mut path = PathBuf::new();
    for part in normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
    {
        if part == ".."
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c))
            || reserved_windows_name(part)
        {
            return Err(unsafe_name(name));
        }
        path.push(part);
    }
    Ok(path)
}

fn reserved_windows_name(name: &str) -> bool {
    let base = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end_matches(' ')
        .to_uppercase();
    matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || base
        .strip_prefix("COM")
        .or_else(|| base.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}

fn unsafe_name(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("Unsafe archive entry path: {name}"),
    )
}

/// Holds the absolute root and its ancestors against replacement on Windows.
pub struct ExtractionRoot {
    path: PathBuf,
    _ancestors: Vec<File>,
}

pub struct ExtractionDestination {
    pub path: PathBuf,
    _parents: Vec<File>,
}

impl ExtractionRoot {
    pub fn new(root: &Path) -> io::Result<Self> {
        if !root.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Extraction root must be absolute",
            ));
        }
        let mut ancestors = Vec::new();
        for ancestor in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
            ancestors.push(platform().hold_archive_directory(ancestor)?);
        }
        let path = std::path::absolute(root)?;
        Ok(Self {
            path,
            _ancestors: ancestors,
        })
    }

    pub fn destination(&self, name: &str, directory: bool) -> io::Result<ExtractionDestination> {
        let relative = relative_archive_path(name)?;
        if relative.as_os_str().is_empty() && !directory {
            return Err(unsafe_name(name));
        }
        let directories = if directory {
            relative.as_path()
        } else {
            relative.parent().ok_or_else(|| unsafe_name(name))?
        };
        let mut parents = Vec::new();
        let mut current = self.path.clone();
        for component in directories.components() {
            current.push(component);
            // Native Windows lookup refuses every reparse component atomically;
            // share flags alone do not block attribute-only reparse mutation.
            parents.push(platform().ensure_archive_directory(&current)?);
        }
        Ok(ExtractionDestination {
            path: self.path.join(relative),
            _parents: parents,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_windows_prefixes_streams_devices_and_ambiguous_components() {
        for name in [
            "../outside",
            "a/../C:victim.txt",
            "a/C:victim.txt",
            "C:/outside",
            "\\\\server\\share\\file",
            "file:extra",
            "a/.. /file",
            "a./file",
            "NUL.txt",
            "COM1",
            "LPT¹.txt",
            "a\0b",
        ] {
            assert!(relative_archive_path(name).is_err(), "accepted {name:?}");
        }
        assert_eq!(
            relative_archive_path("./école\\folder/file.txt").unwrap(),
            Path::new("école").join("folder/file.txt")
        );
    }

    #[test]
    fn rejects_a_parent_that_is_not_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("parent"), b"file").unwrap();
        let root = ExtractionRoot::new(dir.path()).unwrap();
        assert!(root.destination("parent/file", false).is_err());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn held_parents_cannot_be_renamed_during_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let root = ExtractionRoot::new(dir.path()).unwrap();
        let destination = root.destination("parent/file", false).unwrap();
        assert!(fs::rename(dir.path().join("parent"), dir.path().join("replaced")).is_err());
        drop(destination);
        drop(root);
        fs::rename(dir.path().join("parent"), dir.path().join("replaced")).unwrap();
    }
}
