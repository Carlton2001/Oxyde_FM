//! File clipboard domain: copy or cut a selection of files, then paste it elsewhere.

/// Calls may block on the system clipboard: run them on a background worker.
pub trait FileClipboard: Send + Sync {
    /// Transfers selected file paths; `cut` asks the paste to move them.
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<(), String>;
    /// Reads file paths and their copy/move intent.
    fn get_files(&self) -> Result<(Vec<String>, bool), String>;
    /// Checks whether files are offered, without opening or reading the clipboard.
    /// Paste must still validate its payload.
    fn has_file_format(&self) -> bool;
    /// The system clipboard replaced the files since they were set.
    fn discard(&self) {}
}
