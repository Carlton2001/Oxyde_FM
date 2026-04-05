use std::collections::HashSet;
use once_cell::sync::Lazy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconType {
    Folder,
    File,
    FileText,
    FileImage,
    FileVideo,
    Music,
    Package,
    Archive,
    Disc,
    FileCode,
    Link,
    ExternalLink,
    FileSpreadsheet,
    FileStack,
    Database,
    Terminal,
    Key,
    ShieldCheck,
    Box,
    HardDrive,
    Network,
    Globe,
}

impl IconType {
    pub fn path(&self) -> &'static str {
        match self {
            Self::Folder => "icons/folder.svg",
            Self::File => "icons/file.svg",
            Self::FileText => "icons/file-text.svg",
            Self::FileImage => "icons/file-image.svg",
            Self::FileVideo => "icons/file-video.svg",
            Self::Music => "icons/music.svg",
            Self::Package => "icons/package.svg",
            Self::Archive => "icons/archive.svg",
            Self::Disc => "icons/disc.svg",
            Self::FileCode => "icons/file-code.svg",
            Self::Link => "icons/link.svg",
            Self::ExternalLink => "icons/external-link.svg",
            Self::FileSpreadsheet => "icons/file-spreadsheet.svg",
            Self::FileStack => "icons/file-stack.svg",
            Self::Database => "icons/database.svg",
            Self::Terminal => "icons/terminal.svg",
            Self::Key => "icons/key.svg",
            Self::ShieldCheck => "icons/shield-check.svg",
            Self::Box => "icons/box.svg",
            Self::HardDrive => "icons/hard-drive.svg",
            Self::Network => "icons/network.svg",
            Self::Globe => "icons/globe.svg",
        }
    }
}

pub static IMAGE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif"]);
    s
});

pub static PREVIEWABLE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico"]);
    s
});

pub static PREVIEWABLE_VIDEO_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["mp4", "webm", "ogv", "mov", "m4v", "3gp", "mp4v"]);
    s
});

pub static AUDIO_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["mp3", "wav", "ogg", "flac", "aac", "m4a", "wma", "mid", "midi"]);
    s
});

pub static VIDEO_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["mp4", "webm", "ogv", "mov", "m4v", "3gp", "mp4v", "mkv", "avi", "wmv", "flv", "mpg", "mpeg"]);
    s
});

pub static EXECUTABLE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["exe", "msi", "msix", "appx", "bat", "cmd", "ps1", "sh", "jar", "com"]);
    s
});

pub static ARCHIVE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "cab", "tgz", "zst", "tbz2", "tzst", "txz"]);
    s
});

pub static PDF_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["pdf"]);
    s
});

pub static DISK_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["iso", "img", "vmdk", "vhd", "vhdx"]);
    s
});

pub static CODE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend([
        "js", "ts", "tsx", "jsx", "html", "css", "scss", "sass", "less", "wasm",
        "rs", "py", "java", "cpp", "c", "h", "hpp", "cs", "go", "php", "rb", "swift", "kt", "dart",
        "sql", "sh", "bat", "ps1", "cmd", "vbs", "pl", "lua", "r", "m", "f90", "f", "jl", "scala",
        "elm", "erl", "ex", "exs", "clj", "cljs", "lisp", "scm", "ml", "mli", "asm", "s",
        "json", "xml", "yaml", "yml", "md", "toml", "ini", "cfg", "conf", "config", "env",
        "graphql", "gql", "proto", "dockerfile", "makefile", "cmake", "sln", "csproj", "vcxproj"
    ]);
    s
});

pub static TEXT_BASE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["txt", "log", "nfo", "inf", "ini", "cfg", "conf", "config", "bak", "tmp", "csv", "tsv"]);
    s
});

pub static WORD_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["doc", "docx", "odt", "rtf", "pages", "dot", "dotx"]);
    s
});

pub static EXCEL_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["xls", "xlsx", "ods", "csv", "tsv", "numbers", "xlsm"]);
    s
});

pub static POWERPOINT_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["ppt", "pptx", "odp", "key", "pps", "ppsx"]);
    s
});

pub static DATABASE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["accdb", "mdb", "db", "sqlite", "sqlite3"]);
    s
});

pub static CERT_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["pem", "crt", "cer", "key", "p12", "pfx", "pub"]);
    s
});

pub static SYSTEM_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.extend(["dll", "sys", "ocx", "drv", "cpl", "scr"]);
    s
});

pub fn get_icon_type(name: &str, is_dir: bool, path: Option<&str>) -> IconType {
    if is_dir {
        return IconType::Folder;
    }

    if let Some(path) = path {
        // Drive roots (C:\)
        if path.len() <= 3 && path.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) && (path.ends_with(":\\") || path.ends_with(":")) {
             return IconType::HardDrive;
        }
    }

    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if IMAGE_EXTENSIONS.contains(ext.as_str()) {
        return IconType::FileImage;
    }
    if AUDIO_EXTENSIONS.contains(ext.as_str()) {
        return IconType::Music;
    }
    if VIDEO_EXTENSIONS.contains(ext.as_str()) {
        return IconType::FileVideo;
    }
    if WORD_EXTENSIONS.contains(ext.as_str()) {
        return IconType::FileText;
    }
    if EXCEL_EXTENSIONS.contains(ext.as_str()) {
        return IconType::FileSpreadsheet;
    }
    if POWERPOINT_EXTENSIONS.contains(ext.as_str()) {
        return IconType::FileStack;
    }
    if DATABASE_EXTENSIONS.contains(ext.as_str()) {
        return IconType::Database;
    }
    if CERT_EXTENSIONS.contains(ext.as_str()) {
        return IconType::Key;
    }
    if SYSTEM_EXTENSIONS.contains(ext.as_str()) {
        return IconType::ShieldCheck;
    }
    if ext == "msix" || ext == "appx" {
        return IconType::Box;
    }
    if EXECUTABLE_EXTENSIONS.contains(ext.as_str()) {
        return IconType::Package;
    }
    if ["bat", "cmd", "ps1", "sh"].contains(&ext.as_str()) {
        return IconType::Terminal;
    }
    if ARCHIVE_EXTENSIONS.contains(ext.as_str()) {
        return IconType::Archive;
    }
    if PDF_EXTENSIONS.contains(ext.as_str()) {
        return IconType::FileText; // PDF often uses file-text or dedicated icon
    }
    if DISK_EXTENSIONS.contains(ext.as_str()) {
        return IconType::Disc;
    }
    if CODE_EXTENSIONS.contains(ext.as_str()) {
        return IconType::FileCode;
    }
    if TEXT_BASE_EXTENSIONS.contains(ext.as_str()) {
        return IconType::FileText;
    }
    if ext == "lnk" {
        return IconType::ExternalLink;
    }
    if ["url", "website", "webloc"].contains(&ext.as_str()) {
        return IconType::Link;
    }

    IconType::File
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewType {
    None,
    Image,
    Video,
    Pdf,
    Office,
    Text,
    Html,
}

pub fn get_preview_type(name: &str) -> PreviewType {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if PREVIEWABLE_EXTENSIONS.contains(ext.as_str()) {
        return PreviewType::Image;
    }
    if PREVIEWABLE_VIDEO_EXTENSIONS.contains(ext.as_str()) {
        return PreviewType::Video;
    }
    if PDF_EXTENSIONS.contains(ext.as_str()) {
        return PreviewType::Pdf;
    }
    if PDF_EXTENSIONS.contains(ext.as_str()) {
        return PreviewType::Pdf;
    }
    if ["docx", "xlsx", "pptx", "odt", "ods", "odp"].contains(&ext.as_str()) {
        return PreviewType::Office;
    }
    if ext == "html" || ext == "htm" {
        return PreviewType::Html;
    }
    if TEXT_BASE_EXTENSIONS.contains(ext.as_str()) || CODE_EXTENSIONS.contains(ext.as_str()) {
        return PreviewType::Text;
    }

    PreviewType::None
}
