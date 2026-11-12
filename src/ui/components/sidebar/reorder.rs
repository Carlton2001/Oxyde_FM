//! Pinned folder reorder layer: it captures the pointer above the list, so the
//! other rows lose their hover, and floats the moved row like a dragged tab.
use super::sidebar_entry_h;
use crate::ui::app::OxydeApp;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// A reorderable row in list coordinates: top, height and path.
pub(super) struct Slot {
    pub(super) top: f32,
    pub(super) height: f32,
    pub(super) path: String,
}

/// The insertion point nearest to `y`: before a row, or after the last one.
fn target(slots: &[Slot], y: f32) -> Option<(String, bool)> {
    let first = slots.first()?;
    if y < first.top {
        return Some((first.path.clone(), false));
    }
    let index = slots.iter().rposition(|slot| slot.top <= y)?;
    let slot = &slots[index];
    if y < slot.top + slot.height / 2.0 {
        return Some((slot.path.clone(), false));
    }
    // Between two rows, a single marker: the top edge of the next one.
    match slots.get(index + 1) {
        Some(next) => Some((next.path.clone(), false)),
        None => Some((slot.path.clone(), true)),
    }
}

pub(super) fn overlay(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    slots: Vec<Slot>,
    scroll: ScrollHandle,
    theme: OxydeTheme,
) -> Option<AnyElement> {
    let reorder = view.pinned_reorder.as_ref()?;
    let name = view.state.pinned_items.iter().find(|item| item.path == reorder.path)?.name.clone();
    let row_h = sidebar_entry_h();
    // The floating row keeps the list's column and only follows the pointer vertically.
    // Shifted right so the insertion marker stays visible on its left; it may overflow the list.
    let list = scroll.bounds();
    let shift = crate::ui::scale::rem_pixels(rems(1.5));
    let ghost = div()
        .w(list.size.width)
        .px_3()
        .h(px(row_h))
        .flex()
        .items_center()
        .gap_2()
        .bg(theme.tab_active)
        .rounded_sm()
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .text_color(theme.text)
        .opacity(0.95)
        .shadow(crate::ui::style::rem_shadow(true))
        .child(svg().path("icons/pin.svg").size_4().flex_none().text_color(theme.accent))
        .child(name);
    let (move_handle, up_handle, out_handle) = (handle.clone(), handle.clone(), handle);
    Some(
        div()
            .id("sidebar-pinned-reorder")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_move(move |event, _, cx| {
                let bounds = scroll.bounds();
                let y = (event.position.y - bounds.top() - scroll.offset().y).as_f32();
                let target = target(&slots, y);
                move_handle.update(cx, |this, cx| this.move_pinned_reorder(event.position, target, cx));
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| up_handle.update(cx, |this, cx| this.release_pinned(true, cx)))
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| out_handle.update(cx, |this, cx| this.release_pinned(false, cx)))
            .child(deferred(
                anchored()
                    .position(point(list.left() + shift, reorder.pointer.y - px(row_h / 2.0)))
                    .child(ghost),
            ))
            .into_any_element(),
    )
}

#[cfg(test)]
#[path = "tests/reorder_tests.rs"]
mod tests;
