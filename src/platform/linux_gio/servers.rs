//! Recent servers, shared with Nautilus: the GTK bookmark file it reads and writes in
//! `$XDG_CONFIG_HOME/gtk-4.0/servers`.
use super::library::{gio, GError, Gio};
use super::{borrowed, taken, text};
use std::ffi::{c_char, c_void};
use std::path::PathBuf;

/// A server reached before, by the URI that opened it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::platform) struct RecentServer {
    pub uri: String,
    pub title: Option<String>,
}

fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|config| config.join("gtk-4.0").join("servers"))
}

/// The bookmark file, loaded when present, freed when dropped. `whole` unless a file was
/// there that could not be read: saving would then replace every server it lists.
struct Bookmarks(&'static Gio, *mut c_void, bool);

impl Bookmarks {
    fn load(gio: &'static Gio, path: &std::path::Path) -> Self {
        // SAFETY: a new bookmark file; a missing or unreadable file leaves it empty.
        let (bookmarks, failure) = unsafe {
            let bookmarks = (gio.g_bookmark_file_new)();
            let mut error: *mut GError = std::ptr::null_mut();
            (gio.g_bookmark_file_load_from_file)(bookmarks, path_text(path).as_ptr(), &mut error);
            let failure = (!error.is_null()).then(|| borrowed((*error).message));
            free(gio, error);
            (bookmarks, failure)
        };
        // GLib refuses an empty file, which lists nothing to lose.
        let absent = std::fs::metadata(path).map_or_else(|error| error.kind() == std::io::ErrorKind::NotFound, |file| file.len() == 0);
        let whole = failure.is_none() || absent;
        if let (Some(failure), false) = (failure, whole) {
            log::warn!("Cannot read recent servers, they are left unchanged: {failure}");
        }
        Self(gio, bookmarks, whole)
    }

    /// Writes the file unless it was read partly or not at all.
    fn save(&self, path: &std::path::Path) {
        if !self.2 {
            return;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut error: *mut GError = std::ptr::null_mut();
        // SAFETY: a live bookmark file; the error is read then freed.
        unsafe {
            if (self.0.g_bookmark_file_to_file)(self.1, path_text(path).as_ptr(), &mut error) == 0 && !error.is_null() {
                log::warn!("Cannot save recent servers: {}", borrowed((*error).message));
            }
            free(self.0, error);
        }
    }
}

impl Drop for Bookmarks {
    fn drop(&mut self) {
        // SAFETY: created by `load`, freed once.
        unsafe { (self.0.g_bookmark_file_free)(self.1) }
    }
}

unsafe fn free(gio: &Gio, error: *mut GError) {
    if !error.is_null() {
        // SAFETY: an error GLib allocated for the caller.
        unsafe { (gio.g_error_free)(error) }
    }
}

fn path_text(path: &std::path::Path) -> std::ffi::CString {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap_or_default()
}

/// In the order of the file, which keeps the first visit.
pub(in crate::platform) fn recent() -> Vec<RecentServer> {
    let (Some(gio), Some(path)) = (gio(), file()) else { return Vec::new() };
    let bookmarks = Bookmarks::load(gio, &path);
    let mut count = 0;
    // SAFETY: the URI list is read then freed; each title is copied then freed.
    unsafe {
        let uris = (gio.g_bookmark_file_get_uris)(bookmarks.1, &mut count);
        if uris.is_null() {
            return Vec::new();
        }
        let servers = (0..count).map(|index| {
            let uri: *const c_char = *uris.add(index);
            let mut error: *mut GError = std::ptr::null_mut();
            let title = taken(gio, (gio.g_bookmark_file_get_title)(bookmarks.1, uri, &mut error))
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .filter(|title| !title.is_empty());
            free(gio, error);
            RecentServer { uri: borrowed(uri), title }
        }).collect();
        (gio.g_strfreev)(uris);
        servers
    }
}

/// Remembers a location the user connected to.
pub(in crate::platform) fn remember(uri: &str, title: Option<&str>) {
    let (Some(gio), Some(path)) = (gio(), file()) else { return };
    let bookmarks = Bookmarks::load(gio, &path);
    let uri = text(uri);
    // SAFETY: a live bookmark file; GLib copies the strings.
    unsafe {
        // Creates the item, or marks it visited now, as GTK's places view does.
        (gio.g_bookmark_file_add_application)(bookmarks.1, uri.as_ptr(), c"Oxyde".as_ptr(), c"oxyde %u".as_ptr());
        if let Some(title) = title {
            (gio.g_bookmark_file_set_title)(bookmarks.1, uri.as_ptr(), text(title).as_ptr());
        }
    }
    bookmarks.save(&path);
}

pub(in crate::platform) fn forget(uri: &str) {
    let (Some(gio), Some(path)) = (gio(), file()) else { return };
    let bookmarks = Bookmarks::load(gio, &path);
    let mut error: *mut GError = std::ptr::null_mut();
    // SAFETY: a live bookmark file; an unknown URI only sets the error.
    let removed = unsafe { (gio.g_bookmark_file_remove_item)(bookmarks.1, text(uri).as_ptr(), &mut error) != 0 };
    unsafe { free(gio, error) };
    if removed {
        bookmarks.save(&path);
    }
}
