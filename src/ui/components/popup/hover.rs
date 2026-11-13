//! Hover intent: an action that runs once the pointer has rested on its target, as GTK
//! opens a submenu. Crossing a row on the way to an open submenu stays harmless, since
//! reaching the submenu cancels what that row scheduled. One intent at a time: menus
//! never nest two hierarchies.
use gpui::*;
use std::time::Duration;

pub(crate) const DELAY: Duration = Duration::from_millis(250);

#[derive(Default)]
struct Intent {
    generation: u64,
    target: Option<SharedString>,
    /// The pointer's last position in the menu, for the direction of its next move.
    last: Option<Point<Pixels>>,
}
impl Global for Intent {}

/// Runs `action` after [`DELAY`] unless another intent or [`cancel`] comes first.
/// Moving over the same target again keeps the running delay.
pub(crate) fn intend(
    target: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
    action: impl FnOnce(&mut Window, &mut App) + 'static,
) {
    let target = target.into();
    let intent = cx.default_global::<Intent>();
    if intent.target.as_ref() == Some(&target) {
        return;
    }
    intent.generation += 1;
    intent.target = Some(target);
    let generation = intent.generation;
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(DELAY).await;
            let _ = cx.update(|window, cx| {
                let intent = cx.default_global::<Intent>();
                if intent.generation == generation {
                    intent.target = None;
                    action(window, cx);
                }
            });
        })
        .detach();
}

/// Drops the pending intent, if any.
pub(crate) fn cancel(cx: &mut App) {
    let intent = cx.default_global::<Intent>();
    if intent.target.take().is_some() {
        intent.generation += 1;
    }
}

/// Records the pointer at `position` and returns where it was on the previous move,
/// so a menu can tell a move toward its submenu from a rest on another row.
pub(crate) fn moved(position: Point<Pixels>, cx: &mut App) -> Option<Point<Pixels>> {
    cx.default_global::<Intent>().last.replace(position)
}
