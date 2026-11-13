//! How a window looks when it enters or leaves the backdrop. On Linux, title bar content and the
//! window shadow fade like GNOME and KDE headers; on Windows, the title and caption symbols turn
//! grey at once, as native title bars do.

use gpui::{App, Window, WindowId};
use std::{cell::RefCell, collections::HashMap, time::{Duration, Instant}};

/// Opacity of title bar content while the window is inactive, matching libadwaita's backdrop header bar.
const BACKDROP_OPACITY: f32 = 0.5;
const BACKDROP_FADE: Duration = Duration::from_millis(200);

#[derive(Clone, Copy)]
struct Fade {
    active: bool,
    from: f32,
    started: Instant,
}

impl Fade {
    fn target(active: bool) -> f32 {
        if active { 1.0 } else { 0.0 }
    }

    /// Activity at `now`, and whether the fade is still running.
    fn activity(&self, now: Instant) -> (f32, bool) {
        let t = (now.duration_since(self.started).as_secs_f32() / BACKDROP_FADE.as_secs_f32()).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        (self.from + (Self::target(self.active) - self.from) * eased, t < 1.0)
    }
}

thread_local! {
    /// Modal dialogs that act as part of their owner, like libadwaita's AdwDialog: while one
    /// looks active, its owner keeps an active header. Each holds its owner and whether the
    /// dialog looks active, refreshed by [`dialog_activation`].
    static ATTACHED: RefCell<HashMap<WindowId, (WindowId, bool)>> = RefCell::new(HashMap::new());

    /// Only windows that are inactive or fading keep an entry.
    static FADES: RefCell<HashMap<WindowId, Fade>> = RefCell::new(HashMap::new());
}

/// Register `dialog` as a modal part of `owner` until [`detach`].
pub(crate) fn attach(dialog: WindowId, owner: WindowId) {
    ATTACHED.with_borrow_mut(|attached| attached.insert(dialog, (owner, false)));
}

/// Record whether an attached `dialog` looks active, from its activation observer, before its
/// owner is refreshed.
pub(crate) fn dialog_activation(dialog: &Window) {
    let id = dialog.window_handle().window_id();
    let active = dialog.is_window_activated();
    ATTACHED.with_borrow_mut(|attached| {
        if let Some(entry) = attached.get_mut(&id) {
            entry.1 = active;
        }
    });
}

pub(crate) fn detach(dialog: WindowId) {
    ATTACHED.with_borrow_mut(|attached| attached.remove(&dialog));
}

fn attached_dialog_active(owner: WindowId) -> bool {
    ATTACHED.with_borrow(|attached| attached.values().any(|&(of, active)| of == owner && active))
}

/// Whether the window looks active: as the window manager shows it, which survives the
/// keyboard grab of an interactive move. Its own open menus hold the keyboard without making
/// the window look inactive, and so do its attached dialogs.
fn looks_active(window: &Window, cx: &App) -> bool {
    window.is_window_activated()
        || super::popup::has_interactive_menu(window, cx)
        || attached_dialog_active(window.window_handle().window_id())
}

/// How active the window looks for this frame: 1 when active, 0 in the backdrop. Linux eases
/// between the two unless motion is reduced; Windows switches at once.
pub(crate) fn activity(window: &Window, cx: &App) -> f32 {
    let active = looks_active(window, cx);
    let id = window.window_handle().window_id();
    if !cfg!(target_os = "linux") || cx.reduce_motion() {
        FADES.with_borrow_mut(|fades| fades.remove(&id));
        return Fade::target(active);
    }
    let now = Instant::now();
    let (activity, running) = FADES.with_borrow_mut(|fades| {
        let current = fades.get(&id).map_or(1.0, |fade| fade.activity(now).0);
        let previous = fades.get(&id).is_none_or(|fade| fade.active);
        if previous != active {
            // Forget the windows closed meanwhile, which may have closed inactive.
            let open = cx.windows();
            fades.retain(|window, _| open.iter().any(|handle| handle.window_id() == *window));
            fades.insert(id, Fade { active, from: current, started: now });
        }
        let result = fades.get(&id).map_or((1.0, false), |fade| fade.activity(now));
        if active && !result.1 {
            fades.remove(&id);
        }
        result
    });
    if running {
        window.request_animation_frame();
    }
    activity
}

/// Opacity of the title bar content for this frame: dimmed on Linux only.
pub(crate) fn content_opacity(window: &Window, cx: &App) -> f32 {
    if !cfg!(target_os = "linux") {
        return 1.0;
    }
    BACKDROP_OPACITY + (1.0 - BACKDROP_OPACITY) * activity(window, cx)
}

/// Whether the title and caption symbols turn to secondary text, as on inactive Windows
/// title bars. Linux dims the whole content instead.
pub(crate) fn muted_title(window: &Window, cx: &App) -> bool {
    cfg!(target_os = "windows") && activity(window, cx) < 1.0
}

#[cfg(test)]
#[path = "backdrop/tests/fade.rs"]
mod tests;
