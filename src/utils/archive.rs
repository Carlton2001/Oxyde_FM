use std::path::{Path, PathBuf};

pub use super::archive_codec::Codec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZip,
    Tar,
    TarGz,
    TarXz,
    TarZst,
    TarBz2,
    TarLz4,
    TarLzma,
    /// One compressed file (`notes.txt.gz`), shown as an archive holding it.
    Stream(Codec),
    Rar,
    Iso,
}

const TAR_SUFFIXES: [(&str, ArchiveFormat); 14] = [
    (".tar.gz", ArchiveFormat::TarGz),
    (".tgz", ArchiveFormat::TarGz),
    (".tar.xz", ArchiveFormat::TarXz),
    (".txz", ArchiveFormat::TarXz),
    (".tar.zst", ArchiveFormat::TarZst),
    (".tzst", ArchiveFormat::TarZst),
    (".tar.bz2", ArchiveFormat::TarBz2),
    (".tbz2", ArchiveFormat::TarBz2),
    (".tbz", ArchiveFormat::TarBz2),
    (".tar.lz4", ArchiveFormat::TarLz4),
    (".tlz4", ArchiveFormat::TarLz4),
    (".tar.lzma", ArchiveFormat::TarLzma),
    (".tlz", ArchiveFormat::TarLzma),
    (".tar", ArchiveFormat::Tar),
];

impl ArchiveFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?.to_lowercase();
        // Compound TAR suffixes first: `.gz` alone is a single compressed file.
        if let Some((_, format)) = TAR_SUFFIXES.iter().find(|(suffix, _)| name.ends_with(suffix)) {
            return Some(*format);
        }
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "zip" => Some(ArchiveFormat::Zip),
            "7z" => Some(ArchiveFormat::SevenZip),
            "rar" => Some(ArchiveFormat::Rar),
            "iso" | "img" => Some(ArchiveFormat::Iso),
            _ => Codec::from_extension(&ext).map(ArchiveFormat::Stream),
        }
    }

    /// Compression wrapping a TAR stream; `None` for plain TAR and non-TAR formats.
    pub fn tar_codec(self) -> Option<Codec> {
        Some(match self {
            Self::TarGz => Codec::Gzip,
            Self::TarXz => Codec::Xz,
            Self::TarZst => Codec::Zstd,
            Self::TarBz2 => Codec::Bzip2,
            Self::TarLz4 => Codec::Lz4,
            Self::TarLzma => Codec::Lzma,
            _ => return None,
        })
    }

    pub fn is_tar(self) -> bool {
        self == Self::Tar || self.tar_codec().is_some()
    }

    /// Formats that can be created and rebuilt through a workspace.
    pub fn writable(self) -> bool {
        matches!(self, Self::Zip | Self::SevenZip)
            || (self.is_tar() && self.tar_codec().is_none_or(Codec::writable))
    }
}

/// Name of the only member of a single compressed file: its name without the suffix.
pub fn stream_member_name(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    (!stem.is_empty() && stem != "." && stem != "..").then(|| stem.to_owned())
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
    ArchiveFormat::from_path(path).is_some()
}

pub(crate) fn writable_format(path: &Path) -> bool {
    ArchiveFormat::from_path(path).is_some_and(ArchiveFormat::writable)
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

#[cfg(test)]
#[path = "archive/tests/formats.rs"]
mod format_tests;
