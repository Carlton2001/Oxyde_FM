//! Oxyde's own order for pinned folders, kept apart from the system file
//! manager's order, and the sidebar gesture that reorders them.
use super::OxydeApp;
use gpui::*;
use oxyde_lib::models::QuickAccessItem;

/// Pointer travel, in pixels, that turns a press into a move, as for tabs.
const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PinnedReorder {
    pub(crate) path: String,
    /// Row the folder would land next to, and whether it goes after it; `None` until
    /// the pointer moves.
    pub(crate) target: Option<(String, bool)>,
    /// Window position of the pointer, where the floating row follows it.
    pub(crate) pointer: Point<Pixels>,
}

/// Saved paths first, in their saved order; others keep the system order after them.
pub(crate) fn apply_order(items: Vec<QuickAccessItem>, order: &[String]) -> Vec<QuickAccessItem> {
    let mut items: Vec<(usize, QuickAccessItem)> = items
        .into_iter()
        .map(|item| (order.iter().position(|path| *path == item.path).unwrap_or(usize::MAX), item))
        .collect();
    items.sort_by_key(|(rank, _)| *rank);
    items.into_iter().map(|(_, item)| item).collect()
}

/// Moves `path` before or after `target`, or returns `None` when nothing changes.
pub(crate) fn moved(items: &[QuickAccessItem], path: &str, target: &str, after: bool) -> Option<Vec<QuickAccessItem>> {
    if path == target {
        return None;
    }
    let mut moved = items.to_vec();
    let from = moved.iter().position(|item| item.path == path)?;
    let item = moved.remove(from);
    let to = moved.iter().position(|item| item.path == target)? + usize::from(after);
    moved.insert(to, item);
    (to != from).then_some(moved)
}

impl OxydeApp {
    pub(crate) fn press_pinned(&mut self, path: String, pointer: Point<Pixels>) {
        self.pinned_press = Some((path, pointer));
    }

    /// Starts the reorder once the pressed folder has moved far enough.
    pub(crate) fn drag_pinned(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((_, origin)) = &self.pinned_press else { return };
        let travel = pointer - *origin;
        if travel.x.as_f32().hypot(travel.y.as_f32()) < DRAG_THRESHOLD {
            return;
        }
        let Some((path, _)) = self.pinned_press.take() else { return };
        self.pinned_reorder = Some(PinnedReorder { path, target: None, pointer });
        cx.notify();
    }

    pub(crate) fn move_pinned_reorder(&mut self, pointer: Point<Pixels>, target: Option<(String, bool)>, cx: &mut Context<Self>) {
        if let Some(reorder) = &mut self.pinned_reorder {
            reorder.pointer = pointer;
            // Over its own slot too: the room stays open and releasing changes nothing.
            reorder.target = target;
            cx.notify();
        }
    }

    /// Ends the press or the reorder; `commit` applies the hovered position.
    pub(crate) fn release_pinned(&mut self, commit: bool, cx: &mut Context<Self>) {
        self.pinned_press = None;
        let Some(reorder) = self.pinned_reorder.take() else { return };
        cx.notify();
        let Some((target, after)) = reorder.target.filter(|_| commit) else { return };
        let Some(items) = moved(&self.state.pinned_items, &reorder.path, &target, after) else { return };
        self.config.pinned_order = items.iter().map(|item| item.path.clone()).collect();
        self.state.pinned_items = items;
        self.state.refresh_sidebar();
        self.refresh_system_menus(cx);
        self.save_preferences(cx);
    }
}

#[cfg(test)]
#[path = "pinned_order/tests/order.rs"]
mod tests;
