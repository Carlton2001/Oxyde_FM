use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum ArchiveFormat {
    Zip,
    SevenZip,
    Tar,
    TarGz,
    TarXz,
    TarZst,
    TarBz2,
    Rar,
    Iso,
}

impl ArchiveFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "zip" => Some(ArchiveFormat::Zip),
            "7z" => Some(ArchiveFormat::SevenZip),
            "tar" => Some(ArchiveFormat::Tar),
            "zst" | "tzst" => Some(ArchiveFormat::TarZst),
            "gz" | "tgz" => Some(ArchiveFormat::TarGz),
            "xz" | "txz" => Some(ArchiveFormat::TarXz),
            "bz2" | "tbz2" => Some(ArchiveFormat::TarBz2),
            "rar" => Some(ArchiveFormat::Rar),
            "iso" | "img" => Some(ArchiveFormat::Iso),
            _ => {
                let name = path.file_name()?.to_str()?.to_lowercase();
                if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
                    Some(ArchiveFormat::TarGz)
                } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
                    Some(ArchiveFormat::TarXz)
                } else if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
                    Some(ArchiveFormat::TarZst)
                } else if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
                    Some(ArchiveFormat::TarBz2)
                } else {
                    None
                }
            }
        }
    }
}

pub fn is_archive(path: &Path) -> bool {
    ArchiveFormat::from_path(path).is_some()
}

/// Lexical candidate only: this does not establish that the ancestor is a file.
/// Business operations must use `resolve_virtual_path` on a background worker.
pub fn split_virtual_path(path: &str) -> Option<(PathBuf, String)> {
    let path_buf = PathBuf::from(path);
    let mut current = path_buf.as_path();

    let mut result = None;
    while let Some(parent) = current.parent() {
        if is_archive(current) {
            let archive_path = current.to_path_buf();
            let internal_path = path[current.to_string_lossy().len()..]
                .trim_start_matches(['/', '\\'])
                .replace('\\', "/");
            result = Some((archive_path, internal_path));
        }
        current = parent;
    }
    result
}

/// Resolve the actual archive file, skipping ordinary directories with archive suffixes.
/// Includes the archive root itself (empty internal path). Performs filesystem I/O.
pub fn resolve_virtual_path(path: &Path) -> Option<(PathBuf, String)> {
    path.ancestors().find_map(|candidate| {
        if !is_archive(candidate) || !candidate.is_file() { return None; }
        let internal = path.strip_prefix(candidate).ok()?.to_string_lossy().replace('\\', "/");
        Some((candidate.to_path_buf(), internal))
    })
}

pub(crate) fn navigable_format(path: &Path) -> bool {
    matches!(ArchiveFormat::from_path(path), Some(format) if format != ArchiveFormat::Rar)
}

pub(crate) fn writable_format(path: &Path) -> bool {
    matches!(ArchiveFormat::from_path(path), Some(ArchiveFormat::Zip | ArchiveFormat::SevenZip | ArchiveFormat::Tar | ArchiveFormat::TarZst))
}

/// Archives opened as directories by the native UI (nested archives remain files).
/// Filesystem query: UI rendering uses the origin carried by `FileEntry` instead.
pub fn is_navigable_archive(path: &Path) -> bool {
    navigable_format(path) && path.is_file()
}

pub fn is_writable_archive(path: &Path) -> bool {
    writable_format(path) && path.is_file()
}

#[cfg(test)]
#[path = "archive/tests/path_resolution.rs"]
mod path_resolution_tests;
