//! Plain data exchanged with the native UI-thread drag session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileDropEffect {
    #[default]
    None,
    Copy,
    Move,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDropPhase {
    Enter,
    Over,
    Leave,
    Drop,
}

pub struct NativeFileDrop {
    pub phase: FileDropPhase,
    /// Captured once at Enter; later events refer to that same selection.
    pub paths: Vec<String>,
    /// Physical client coordinates, including on mixed-DPI desktops.
    pub position: [i32; 2],
    pub control: bool,
    pub shift: bool,
    pub can_copy: bool,
    pub can_move: bool,
    /// At Drop only, for files the source provides as streams instead of paths:
    /// `paths` then name them in a staging folder that does not exist yet.
    pub streamed: Option<StreamedFiles>,
}

/// Files another application holds only as streams (an Explorer ZIP folder,
/// Outlook attachments and messages). Dropping it unused aborts the transfer.
pub struct StreamedFiles(pub(super) Box<dyn StreamedSource>);

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(super) trait StreamedSource: Send {
    /// Blocking: writes every file into the staging folder.
    fn extract(self: Box<Self>) -> Result<(Vec<String>, DropStaging), String>;
}

impl StreamedFiles {
    /// Blocking: the top-level items written, and the guard removing them.
    pub fn extract(self) -> Result<(Vec<String>, DropStaging), String> {
        self.0.extract()
    }
}

/// A staging folder removed in the background when the guard is dropped.
pub struct DropStaging(pub(super) std::path::PathBuf);

impl Drop for DropStaging {
    fn drop(&mut self) {
        let path = std::mem::take(&mut self.0);
        if path.as_os_str().is_empty() {
            return;
        }
        let _ = std::thread::Builder::new().name("drop-staging".into()).spawn(move || {
            if let Err(error) = std::fs::remove_dir_all(&path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Cannot remove drop staging {}: {error}", path.display());
                }
            }
        });
    }
}

pub struct FileDragFeedback {
    pub effect: FileDropEffect,
    /// Release/cancellation ends the preview before a potentially slow native Drop.
    pub dismiss_preview: bool,
    pub finished: bool,
    pub cancelled: bool,
    /// Only an explicitly confirmed unoptimized native move needs source cleanup.
    pub delete_sources: bool,
}
