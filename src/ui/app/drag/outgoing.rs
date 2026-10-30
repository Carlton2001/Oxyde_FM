//! Files offered to other applications when a drag leaves the window. GPUI hands the
//! gesture to Wayland, whose compositor then shows no preview of it; the receiver does
//! the copy or the move itself, as from Nautilus. Its X11 backend cannot offer files, so
//! a drag there stays inside Oxyde; Windows has its own OLE session.
use super::super::{FileDragData, OxydeApp};
use gpui::{App, Entity, ExternalDragPayload, FileDragPaths, Window};
use std::path::{Path, PathBuf};

/// Everything or nothing: a partial drop would look complete. Lexical, on the UI thread:
/// archive members need materializing first and names that are not UTF-8 cannot be
/// operated on yet.
fn offered_paths(paths: &[String]) -> Option<Vec<PathBuf>> {
    paths.iter().map(|path| {
        let offered = !path.is_empty()
            && Path::new(path).is_absolute()
            && !oxyde_lib::utils::path_text::is_encoded(path)
            && !oxyde_lib::utils::archive::split_virtual_path(path)
                .is_some_and(|(_, member)| !member.is_empty());
        offered.then(|| PathBuf::from(path))
    }).collect()
}

/// Called at most once per drag, when the pointer leaves the window.
pub(crate) fn resolver(app: Entity<OxydeApp>)
    -> impl Fn(&FileDragData, &mut Window, &mut App) -> Option<ExternalDragPayload> + 'static {
    move |data, _, cx| {
        // Transfers from the trash stay refused: they would split a file from its record.
        let from_trash = app.read(cx).state.find_pane(&data.source_pane_id)
            .is_some_and(|pane| pane.active_tab().path == "trash://");
        if from_trash {
            return None;
        }
        // The flag tells macOS about folders; Wayland only sends URIs.
        let entries = offered_paths(&data.paths)?.into_iter().map(|path| (path, false));
        Some(ExternalDragPayload::Files(FileDragPaths::new(entries)))
    }
}

#[cfg(test)]
#[path = "tests/outgoing.rs"]
mod tests;
