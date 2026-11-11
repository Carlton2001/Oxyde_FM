//! Title bar content fades like GNOME and KDE headers when its window enters or leaves the backdrop.

use gpui::{App, Window, WindowId};
use std::{cell::RefCell, collections::HashMap, time::{Duration, Instant}};

/// Opacity of title bar content while the window is inactive, matching libadwaita's backdrop header bar.
const BACKDROP_OPACITY: f32 = 0.5;
const BACKDROP_FADE: Duration = Duration::from_millis(200);

#[derive(Clone, Copy)]
struct Fade {
    dimmed: bool,
    from: f32,
    started: Instant,
}

impl Fade {
    fn target(dimmed: bool) -> f32 {
        if dimmed { BACKDROP_OPACITY } else { 1.0 }
    }

    fn opacity(&self, now: Instant) -> (f32, bool) {
        let t = (now.duration_since(self.started).as_secs_f32() / BACKDROP_FADE.as_secs_f32()).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        (self.from + (Self::target(self.dimmed) - self.from) * eased, t < 1.0)
    }
}

thread_local! {
    /// Modal dialogs that act as part of their owner, like libadwaita's AdwDialog: while one
    /// has the focus, its owner keeps an active header.
    static ATTACHED: RefCell<HashMap<WindowId, WindowId>> = RefCell::new(HashMap::new());

    /// Only windows that are dimmed or fading keep an entry.
    static FADES: RefCell<HashMap<WindowId, Fade>> = RefCell::new(HashMap::new());
}

/// Register `dialog` as a modal part of `owner` until [`detach`].
pub(crate) fn attach(dialog: WindowId, owner: WindowId) {
    ATTACHED.with_borrow_mut(|attached| attached.insert(dialog, owner));
}

pub(crate) fn detach(dialog: WindowId) {
    ATTACHED.with_borrow_mut(|attached| attached.remove(&dialog));
}

fn attached_dialog_active(owner: WindowId, cx: &App) -> bool {
    cx.active_window().is_some_and(|active| {
        ATTACHED.with_borrow(|attached| attached.get(&active.window_id()) == Some(&owner))
    })
}

/// Opacity of the title bar content for this frame. Its own open menus hold the keyboard
/// without making the window look inactive.
pub(crate) fn content_opacity(window: &Window, cx: &App) -> f32 {
    if !cfg!(target_os = "linux") {
        return 1.0;
    }
    let id = window.window_handle().window_id();
    let dimmed = !window.is_window_active()
        && !super::popup::has_interactive_menu(window, cx)
        && !attached_dialog_active(id, cx);
    let now = Instant::now();
    let (opacity, running) = FADES.with_borrow_mut(|fades| {
        let current = fades.get(&id).map_or((1.0, false), |fade| fade.opacity(now));
        let previous = fades.get(&id).map_or(false, |fade| fade.dimmed);
        if previous != dimmed {
            fades.insert(id, Fade { dimmed, from: current.0, started: now });
        }
        let result = fades.get(&id).map_or((1.0, false), |fade| fade.opacity(now));
        if !dimmed && !result.1 {
            fades.remove(&id);
        }
        result
    });
    if running {
        window.request_animation_frame();
    }
    opacity
}
