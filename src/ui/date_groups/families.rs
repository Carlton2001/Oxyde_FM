use crate::ui::i18n::Message;
use oxyde_lib::models::FileEntry;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileFamily {
    Folders,
    Archives,
    DiskImages,
    Applications,
    Documents,
    Images,
    Video,
    Audio,
    Code,
    Scripts,
    Other,
}
impl FileFamily {
    pub fn message(self) -> Message {
        match self {
            Self::Folders => Message::GroupFolders,
            Self::Archives => Message::GroupArchives,
            Self::DiskImages => Message::GroupDiskImages,
            Self::Applications => Message::GroupApplications,
            Self::Documents => Message::GroupDocuments,
            Self::Images => Message::GroupImages,
            Self::Video => Message::GroupVideo,
            Self::Audio => Message::GroupAudio,
            Self::Code => Message::GroupCode,
            Self::Scripts => Message::GroupScripts,
            Self::Other => Message::GroupOther,
        }
    }
    pub fn for_file(file: &FileEntry) -> Self {
        if file.is_dir {
            return Self::Folders;
        }
        let name = file.name.to_ascii_lowercase();
        let extension = std::path::Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        match extension {
            "iso" | "img" | "vhd" | "vhdx" | "avhd" | "avhdx" | "vmdk" | "vdi" | "qcow"
            | "qcow2" | "dmg" | "nrg" | "sparseimage" => Self::DiskImages,
            "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "zst" | "tgz" | "tbz" | "tbz2"
            | "txz" | "cab" | "lz" | "lzma" | "br" | "arj" | "ace" | "lzh" => Self::Archives,
            "exe" | "msi" | "msix" | "msixbundle" | "appx" | "appxbundle" | "com" | "scr"
            | "appimage" | "deb" | "rpm" | "apk" => Self::Applications,
            "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "xls" | "xlsx" | "ods" | "csv"
            | "tsv" | "ppt" | "pptx" | "odp" | "epub" | "mobi" | "pages" | "numbers" | "key"
            | "md" => Self::Documents,
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "avif" | "heic" | "heif"
            | "tif" | "tiff" | "ico" | "raw" | "cr2" | "nef" | "dng" | "psd" | "xcf" | "jxl" => {
                Self::Images
            }
            "mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv" | "m4v" | "mpg" | "mpeg" | "m2ts"
            | "flv" | "3gp" => Self::Video,
            "mp3" | "flac" | "wav" | "ogg" | "opus" | "m4a" | "aac" | "wma" | "aiff" | "alac"
            | "mid" | "midi" => Self::Audio,
            _ if crate::ui::file_types::is_script_file(&file.name) => Self::Scripts,
            _ if crate::ui::file_types::is_code_file(&file.name) => Self::Code,
            _ => match file.native_kind {
                Some(oxyde_lib::models::file_entry::NativeFileKind::Documents) => Self::Documents,
                Some(oxyde_lib::models::file_entry::NativeFileKind::Images) => Self::Images,
                Some(oxyde_lib::models::file_entry::NativeFileKind::Audio) => Self::Audio,
                Some(oxyde_lib::models::file_entry::NativeFileKind::Video) => Self::Video,
                Some(oxyde_lib::models::file_entry::NativeFileKind::Archives) => Self::Archives,
                Some(oxyde_lib::models::file_entry::NativeFileKind::Applications) => {
                    Self::Applications
                }
                None => Self::Other,
            },
        }
    }
}

#[cfg(test)]
#[path = "tests/families.rs"]
mod tests;
