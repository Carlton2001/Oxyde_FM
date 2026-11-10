//! Windows file clipboard: `CF_HDROP` with `Preferred DropEffect`, in `clipboard.rs`; the shell's Paste
//! as a fallback, in `windows_system_paste.rs`.
use super::FileClipboard;

pub struct WindowsFileClipboard;

impl FileClipboard for WindowsFileClipboard {
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<bool, String> {
        super::clipboard::set_clipboard_files(paths, cut)
            .map(|()| false)
            .map_err(|error| format!("{error:?}"))
    }

    fn get_files(&self, _text: Option<String>) -> Result<(Vec<String>, bool), String> {
        super::clipboard::get_clipboard_files().map_err(|error| format!("{error:?}"))
    }

    fn has_file_format(&self) -> bool {
        super::clipboard::has_file_format() || super::windows_system_paste::available()
    }

    fn paste_with_system(&self, target: &str) -> Result<bool, String> {
        super::windows_system_paste::paste_into(target)
    }
}
