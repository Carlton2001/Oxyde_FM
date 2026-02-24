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

/// Splits a virtual path like C:\path\to\archive.zip\folder into (archive_path, internal_path)
pub fn split_virtual_path(path: &str) -> Option<(PathBuf, String)> {
    let path_buf = PathBuf::from(path);
    let mut current = path_buf.as_path();

    while let Some(parent) = current.parent() {
        if is_archive(current) {
            let archive_path = current.to_path_buf();
            let internal_path = path[current.to_string_lossy().len()..]
                .trim_start_matches(['/', '\\'])
                .replace('\\', "/");
            return Some((archive_path, internal_path));
        }
        current = parent;
    }
    None
}
