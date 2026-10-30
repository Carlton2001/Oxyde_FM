#![allow(clippy::type_complexity, clippy::too_many_arguments)]

pub mod app;
pub mod assets;
pub(crate) mod preview_jobs;
pub(crate) mod preview_disk;
pub(crate) mod audio_covers;
pub(crate) mod preview_budget;
pub(crate) mod memory_diagnostics;
pub(crate) mod image_preview;
pub(crate) mod preview_worker;
pub mod components;
pub mod elements;
pub mod errors;
pub mod date_groups;
pub mod format;
pub(crate) mod file_types;
pub mod i18n;
pub mod scale;
pub mod style;
pub mod theme;
pub mod workspace;
pub(crate) mod window_geometry;

/// Wayland `app_id` and X11 class of every Oxyde window: GNOME finds the desktop
/// entry and icon of that name to group the windows and show them in the dash.
pub const APP_ID: &str = "io.github.Carlton2001.Oxyde";

/// Every titled Oxyde window draws its own frame: asked for server decorations, KDE would
/// add a title bar above Oxyde's. Compositors without client decorations still give theirs.
pub fn window_decorations() -> Option<gpui::WindowDecorations> {
    cfg!(target_os = "linux").then_some(gpui::WindowDecorations::Client)
}
