//! File clipboard domain: copy or cut a selection of files, then paste it elsewhere.

/// Calls may block on the system clipboard: run them on a background worker.
pub trait FileClipboard: Send + Sync {
    /// Transfers selected file paths; `cut` asks the paste to move them. Returns whether
    /// the interface must also write the paths as text: the platform could not offer
    /// them to other applications itself.
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<bool, String>;
    /// Reads file paths and their copy/move intent. `text` is the system clipboard's
    /// text: files offered only as text give way to text copied elsewhere since.
    fn get_files(&self, text: Option<String>) -> Result<(Vec<String>, bool), String>;
    /// Checks whether files are offered, without opening or reading the clipboard.
    /// Paste must still validate its payload.
    fn has_file_format(&self) -> bool;
}
