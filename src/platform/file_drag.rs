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
