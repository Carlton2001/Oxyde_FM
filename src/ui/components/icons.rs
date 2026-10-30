mod colors;
pub use colors::FileIconPalette;

use once_cell::sync::Lazy;
use std::collections::HashSet;

/// Shared by every surface displaying a mounted drive.
pub fn drive_icon_path(drive: &oxyde_lib::models::DriveInfo) -> &'static str {
    if drive.is_usb() { return "icons/usb.svg"; }
    match drive.drive_type.as_str() {
        "cdrom" => "icons/disc.svg",
        "remote" => "icons/network.svg",
        _ => "icons/hard-drive.svg",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconType {
    Folder,
    FolderOpen,
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
    Link2,
    // Windows keeps one icon for symbolic links and junctions.
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    FolderSymlink,
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    FileSymlink,
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    BrokenLink,
    ExternalLink,
    FileSpreadsheet,
    FileStack,
    Database,
    Key,
    ShieldCheck,
    Box,
    HardDrive,
    Network,
    Globe,
    Trash,
    Monitor,
    Smartphone,
    Printer,
}

impl IconType {
    pub fn path(&self) -> &'static str {
        match self {
            Self::Folder => "icons/folder.svg",
            Self::FolderOpen => "icons/folder-open.svg",
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
            Self::Link2 => "icons/link2.svg",
            Self::FolderSymlink => "icons/folder-symlink.svg",
            Self::FileSymlink => "icons/file-symlink.svg",
            Self::BrokenLink => "icons/link-2-off.svg",
            Self::ExternalLink => "icons/external-link.svg",
            Self::FileSpreadsheet => "icons/file-spreadsheet.svg",
            Self::FileStack => "icons/file-stack.svg",
            Self::Database => "icons/database.svg",
            Self::Key => "icons/key.svg",
            Self::ShieldCheck => "icons/shield-check.svg",
            Self::Box => "icons/box.svg",
            Self::HardDrive => "icons/hard-drive.svg",
            Self::Network => "icons/network.svg",
            Self::Globe => "icons/globe.svg",
            Self::Trash => "icons/trash.svg",
            Self::Monitor => "icons/monitor.svg",
            Self::Smartphone => "icons/smartphone.svg",
            Self::Printer => "icons/printer.svg",
        }
    }
}

pub(crate) fn entry_icon(file: &oxyde_lib::models::FileEntry) -> IconType {
    if let Some(details) = &file.network_details {
        let category = details.category.to_lowercase();
        if category.contains("phone") || category.contains("téléphone") || category.contains("mobile") { return IconType::Smartphone; }
        if category.contains("computer") || category.contains("ordinateur") { return IconType::Monitor; }
        if category.contains("printer") || category.contains("imprimante") { return IconType::Printer; }
        if !category.is_empty() { return IconType::Box; }
    }
    if oxyde_lib::commands::io::is_network_server(&file.path) { return IconType::Monitor; }
    #[cfg(not(target_os = "windows"))]
    if file.is_symlink { return link_icon(file.is_dir, file.is_broken_link); }
    get_icon_type(&file.name, file.is_dir, Some(&file.path), file.is_symlink, file.is_junction)
}

/// As in Nautilus, a link shows whether it leads to a folder, a file or nowhere.
#[cfg(not(target_os = "windows"))]
pub(crate) fn link_icon(is_dir: bool, broken: bool) -> IconType {
    if broken {
        IconType::BrokenLink
    } else if is_dir {
        IconType::FolderSymlink
    } else {
        IconType::FileSymlink
    }
}

// ---------------------------------------------------------------------------
// Extension sets — O(1) lookup, mirrors fileIcons.tsx from V1
// ---------------------------------------------------------------------------

pub static IMAGE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif",
    ]
    .into()
});

/// Subset of images that can be previewed inline (tooltip / inspector).
pub static PREVIEWABLE_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico"].into());

pub static PREVIEWABLE_VIDEO_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["mp4", "webm", "ogv", "mov", "m4v", "3gp", "mp4v", "avi"].into());

pub static PREVIEWABLE_OFFICE_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["docx", "xlsx", "pptx", "odt", "ods", "odp"].into());

pub static AUDIO_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "mp3", "wav", "ogg", "flac", "aac", "m4a", "wma", "mid", "midi", "aiff", "aif", "aifc",
        "opus", "oga", "m4b", "mp2", "ac3", "ape", "wv", "tta", "mka", "spx", "au",
    ]
    .into()
});

pub static VIDEO_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "mp4", "webm", "ogv", "mov", "m4v", "3gp", "mp4v", "mkv", "avi", "wmv", "flv", "mpg",
        "mpeg",
    ]
    .into()
});

pub static EXECUTABLE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "exe", "msi", "msix", "appx", "bat", "cmd", "ps1", "sh", "jar", "com",
    ]
    .into()
});

pub static ARCHIVE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "cab", "tgz", "zst", "tbz2", "tzst", "txz",
    ]
    .into()
});

pub static PDF_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| ["pdf"].into());

pub static DISK_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["iso", "img", "vmdk", "vhd", "vhdx"].into());

pub static TEXT_BASE_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "txt", "log", "nfo", "inf", "ini", "cfg", "conf", "config", "bak", "tmp", "csv", "tsv",
    ]
    .into()
});

pub static WORD_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["doc", "docx", "odt", "rtf", "pages", "dot", "dotx"].into());

pub static EXCEL_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["xls", "xlsx", "ods", "csv", "tsv", "numbers", "xlsm"].into());

pub static POWERPOINT_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["ppt", "pptx", "odp", "key", "pps", "ppsx"].into());

pub static DATABASE_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["accdb", "mdb", "db", "sqlite", "sqlite3"].into());

pub static CERT_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["pem", "crt", "cer", "key", "p12", "pfx", "pub"].into());

pub static SYSTEM_EXTENSIONS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["dll", "sys", "ocx", "drv", "cpl", "scr"].into());

// ---------------------------------------------------------------------------
// Icon resolution — mirrors getFileIcon() logic from V1
// ---------------------------------------------------------------------------

pub fn get_icon_type(
    name: &str,
    is_dir: bool,
    path: Option<&str>,
    is_symlink: bool,
    is_junction: bool,
) -> IconType {
    // 1. Virtual / special paths
    if let Some(p) = path {
        if p == "__network_vincinity__" {
            return IconType::Globe;
        }
        if p == "trash://" {
            return IconType::Trash;
        }
        // Windows Shell items (CLSID paths, network vicinity expansions)
        if p.starts_with("::{") || p.starts_with('?') {
            return IconType::Network;
        }
        // UNC server roots (\\SERVER — one component, not a share)
        if p.starts_with("\\\\") {
            let parts: Vec<&str> = p.split('\\').filter(|s| !s.is_empty()).collect();
            if parts.len() <= 1 {
                return IconType::Network;
            }
        }
        // Drive roots (C:\ or C:)
        if p.len() <= 3
            && p.chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false)
            && (p.ends_with(":\\") || p.ends_with(':'))
        {
            return IconType::HardDrive;
        }
    }

    // 2. Symlinks / junctions
    if is_symlink || is_junction {
        return IconType::Link2;
    }

    // 3. Directories
    if is_dir {
        return IconType::Folder;
    }

    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let ext = ext.as_str();

    if IMAGE_EXTENSIONS.contains(ext) {
        return IconType::FileImage;
    }
    if AUDIO_EXTENSIONS.contains(ext) {
        return IconType::Music;
    }
    if VIDEO_EXTENSIONS.contains(ext) {
        return IconType::FileVideo;
    }

    // Office productivity
    if WORD_EXTENSIONS.contains(ext) {
        return IconType::FileText;
    }
    if EXCEL_EXTENSIONS.contains(ext) {
        return IconType::FileSpreadsheet;
    }
    if POWERPOINT_EXTENSIONS.contains(ext) {
        return IconType::FileStack;
    }

    // Specialised types
    if DATABASE_EXTENSIONS.contains(ext) {
        return IconType::Database;
    }
    if CERT_EXTENSIONS.contains(ext) {
        return IconType::Key;
    }
    if SYSTEM_EXTENSIONS.contains(ext) {
        return IconType::ShieldCheck;
    }

    // Executables & installers
    if ext == "msix" || ext == "appx" {
        return IconType::Box;
    }
    if EXECUTABLE_EXTENSIONS.contains(ext) {
        return IconType::Package;
    }

    if ARCHIVE_EXTENSIONS.contains(ext) {
        return IconType::Archive;
    }
    if PDF_EXTENSIONS.contains(ext) {
        return IconType::FileText;
    }
    if DISK_EXTENSIONS.contains(ext) {
        return IconType::Disc;
    }
    if crate::ui::file_types::is_code_file(name) {
        return IconType::FileCode;
    }
    if TEXT_BASE_EXTENSIONS.contains(ext) {
        return IconType::FileText;
    }

    if ext == "lnk" {
        return IconType::ExternalLink;
    }
    if matches!(ext, "url" | "website" | "webloc") {
        return IconType::Link;
    }

    IconType::File
}

// ---------------------------------------------------------------------------
// Preview type resolution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewType {
    None,
    Image,
    Audio,
    Video,
    Pdf,
    Office,
    Text,
}

pub fn get_preview_type(name: &str) -> PreviewType {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let ext = ext.as_str();

    if PREVIEWABLE_EXTENSIONS.contains(ext) {
        return PreviewType::Image;
    }
    if PREVIEWABLE_VIDEO_EXTENSIONS.contains(ext) {
        return PreviewType::Video;
    }
    if AUDIO_EXTENSIONS.contains(ext) {
        return PreviewType::Audio;
    }
    if PDF_EXTENSIONS.contains(ext) {
        return PreviewType::Pdf;
    }
    if PREVIEWABLE_OFFICE_EXTENSIONS.contains(ext) {
        return PreviewType::Office;
    }
    if ext == "html" || ext == "htm" {
        return PreviewType::Text;
    }
    if TEXT_BASE_EXTENSIONS.contains(ext) || crate::ui::file_types::is_code_file(name) {
        return PreviewType::Text;
    }

    PreviewType::None
}

use gpui::prelude::*;
use gpui::*;

struct SystemIconAsset;
#[derive(Default)]
pub(crate) struct SystemFolderImages(Option<[std::sync::Arc<RenderImage>; 2]>);
impl Global for SystemFolderImages {}

/// Before opening the first window, prepare the two shared folder sizes.
pub(crate) fn prepare_system_folder_icons(enabled: bool) -> SystemFolderImages {
    if !enabled || !cfg!(target_os = "windows") { return SystemFolderImages::default(); }
    let load = |large| {
        oxyde_lib::platform::platform().file_icon("oxyde_dir_generic".into(), large).map(|mut image| {
            rgba_to_bgra(&mut image);
            std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(image)]))
        })
    };
    match (load(false), load(true)) {
        (Ok(small), Ok(large)) => SystemFolderImages(Some([small, large])),
        _ => SystemFolderImages::default(),
    }
}

pub(crate) fn invalidate_file_icon(path: &str, cx: &mut App) {
    cx.set_global(SystemFolderImages::default());
    for large in [false, true] {
        cx.remove_asset::<SystemIconAsset>(&(SharedString::from(path.to_string()), large));
        for (is_dir, preserve_path) in [(false, false), (true, false), (false, true)] {
            let source = oxyde_lib::platform::platform().file_icon_source(path, is_dir, preserve_path);
            cx.remove_asset::<SystemIconAsset>(&(SharedString::from(source), large));
        }
    }
}
impl Asset for SystemIconAsset {
    type Source = (SharedString, bool);
    type Output = Result<std::sync::Arc<RenderImage>, ImageCacheError>;
    fn load(
        source: Self::Source,
        _cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let task = crate::ui::preview_jobs::load_icon(move || {
            oxyde_lib::platform::platform()
                .file_icon(source.0.to_string(), source.1)
                .map(|mut image| {
                    rgba_to_bgra(&mut image);
                    image
                })
                .map_err(|e| ImageCacheError::Asset(e.into()))
        });
        async move {
            Ok(std::sync::Arc::new(RenderImage::new(vec![
                image::Frame::new(
                    task.await
                        .map_err(|error| ImageCacheError::Asset(error.into()))?,
                ),
            ])))
        }
    }
}
pub(crate) fn render_file_icon(
    path: String,
    icon: IconType,
    size: Rems,
    theme: &crate::ui::theme::OxydeTheme,
    system: bool,
    large: bool,
) -> AnyElement {
    let color = colors::file_icon_color(&path, icon, theme);
    let fallback = move || {
        if matches!(icon, IconType::Folder | IconType::FolderOpen) {
            let fill_path = if icon == IconType::Folder {
                "icons/folder-fill.svg"
            } else {
                "icons/folder-open-fill.svg"
            };
            return div()
                .relative()
                .size(size)
                .child(svg().path(fill_path).size(size).text_color(color).absolute())
                .child(svg().path(icon.path()).size(size).text_color(color).absolute())
                .into_any_element();
        }
        svg()
            .path(icon.path())
            .size(size)
            .text_color(color)
            .into_any_element()
    };
    if !system || path.starts_with("trash:") || path == "__network_vincinity__" {
        return fallback();
    }
    // Special locations and directories retain their own Shell icons. Ordinary
    // files share the same GPUI asset, including its pending load and GPU image.
    let preserve_path = matches!(icon,
        IconType::Link2 |
        IconType::HardDrive | IconType::Network | IconType::Globe |
        IconType::Trash | IconType::Monitor | IconType::Smartphone | IconType::Printer | IconType::Box
    );
    let source = (
        SharedString::from(oxyde_lib::platform::platform().file_icon_source(&path, matches!(icon, IconType::Folder | IconType::FolderOpen), preserve_path)),
        large,
    );
    img(move |window: &mut Window, cx: &mut App| {
        if source.0.as_ref() == "oxyde_dir_generic" {
            if let Some(images) = cx.try_global::<SystemFolderImages>().and_then(|images| images.0.as_ref()) {
                return Some(Ok(images[usize::from(large)].clone()));
            }
        }
        window.use_asset::<SystemIconAsset>(&source, cx)
    })
        .size(size)
        .object_fit(ObjectFit::Contain)
        .with_fallback(fallback)
        .into_any_element()
}

/// GPUI RenderImage consumes BGRA bytes; decoded PNGs contain RGBA bytes.
fn rgba_to_bgra(image: &mut image::RgbaImage) {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
}

#[cfg(test)]
mod system_icon_tests {
    use super::rgba_to_bgra;

    #[test]
    fn avi_and_mov_are_routed_to_the_video_preview() {
        for name in ["09-video.avi", "MOVIE.AVI", "film.mov", "FILM.MOV"] {
            assert!(matches!(
                super::get_preview_type(name),
                super::PreviewType::Video
            ));
        }
    }

    #[test]
    fn native_icon_colors_are_converted_for_gpui_without_changing_alpha() {
        let mut image = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 205, 40, 128]));
        rgba_to_bgra(&mut image);
        assert_eq!(image.get_pixel(0, 0).0, [40, 205, 255, 128]);
    }

    #[test]
    fn network_categories_choose_device_icons_without_relabeling_normal_folders() {
        use super::{entry_icon, IconType};
        for (category, expected) in [("Ordinateur", IconType::Monitor), ("Computer", IconType::Monitor), ("Téléphones", IconType::Smartphone), ("Phones", IconType::Smartphone), ("Imprimantes", IconType::Printer), ("Autres périphériques", IconType::Box)] {
            let entry = oxyde_lib::models::FileEntry {
                path: "::{device}".into(),
                network_details: Some(oxyde_lib::models::file_entry::NetworkDetails { category: category.into(), ..Default::default() }),
                ..Default::default()
            };
            assert_eq!(entry_icon(&entry), expected);
        }
        let server = if cfg!(target_os = "windows") { "\\\\SERVEUR" } else { "smb://SERVEUR/" };
        let share = if cfg!(target_os = "windows") { "\\\\SERVEUR\\Partage commun" } else { "smb://SERVEUR/Partage commun" };
        assert_eq!(entry_icon(&oxyde_lib::models::FileEntry { path: server.into(), ..Default::default() }), IconType::Monitor);
        assert_eq!(entry_icon(&oxyde_lib::models::FileEntry { path: share.into(), is_dir: true, ..Default::default() }), IconType::Folder);
    }
}
