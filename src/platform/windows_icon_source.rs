//! Stable Shell requests shared by the UI asset cache and native icon caches.

const UNIQUE_EXTENSIONS: &[&str] = &[
    "exe",
    "ico",
    "cur",
    "ani",
    "lnk",
    "url",
    "cpl",
    "msi",
    "msix",
    "appx",
    "bat",
    "cmd",
    "msc",
    "ps1",
    "scr",
    "theme",
    "themepack",
];

pub(super) fn source(path: &str, is_dir: bool, preserve_path: bool) -> String {
    let path = path.replace('/', "\\");
    if preserve_path || path.starts_with("oxyde_ext_") || path == "oxyde_file_generic" {
        return path;
    }
    if is_dir {
        return if special_directory(&path) {
            path
        } else {
            "oxyde_dir_generic".into()
        };
    }
    let name = path.rsplit('\\').next().unwrap_or(&path);
    let extension = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some(ext) if UNIQUE_EXTENSIONS.contains(&ext) => path,
        Some(ext) if !ext.is_empty() => format!("oxyde_ext_{ext}"),
        _ => "oxyde_file_generic".into(),
    }
}

fn special_directory(path: &str) -> bool {
    let path = path.trim_end_matches('\\');
    if path.ends_with(':') || path.starts_with("::{") || path.starts_with('?') {
        return true;
    }
    if path.starts_with("\\\\") && path[2..].split('\\').count() <= 2 {
        return true;
    }
    let lower = path.to_lowercase();
    let parts: Vec<_> = lower.split('\\').collect();
    if let Some(index) = parts.iter().position(|part| *part == "users") {
        if parts.len() == index + 3 {
            return matches!(
                parts[index + 2],
                "documents"
                    | "downloads"
                    | "desktop"
                    | "pictures"
                    | "videos"
                    | "music"
                    | "favorites"
                    | "links"
                    | "onedrive"
                    | "searches"
                    | "contacts"
                    | "3d objects"
                    | "mes documents"
                    | "mes images"
                    | "mes vidéos"
                    | "téléchargements"
                    | "bureau"
                    | "ma musique"
            );
        }
    }
    false
}

#[cfg(test)]
#[path = "windows_icon_source/tests/source.rs"]
mod tests;
