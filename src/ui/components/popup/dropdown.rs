use super::{show, PopupAnchor};
use crate::ui::components::{dropdown::{DropdownItem, GenericDropdown}, scrollbar::ScrollbarState};
use gpui::*;
use std::rc::Rc;

pub(crate) fn text_menu<T: 'static, S: ScrollbarState + 'static>(
    id: impl Into<SharedString>, anchor: PopupAnchor, source: Entity<T>, scroll_owner: Entity<S>,
    items: Vec<DropdownItem>, scroll: UniformListScrollHandle, parent: &mut Window, cx: &mut App,
    alive: impl Fn(&T) -> bool + 'static,
    close: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
) {
    let id = id.into();
    let weak = source.downgrade();
    let dismiss = Rc::new(close);
    let close_menu = dismiss.clone();
    show(id.clone(), anchor, source, parent, cx, alive, move |_, window, cx| {
        let weak = weak.clone();
        let dismiss = dismiss.clone();
        GenericDropdown {
            id: id.clone().into(), items: items.clone(), reserve_icon_space: true, is_context_menu: true,
            bottom_padding: 4.0, scroll_handle: scroll.clone(), h_app: scroll_owner.clone(),
            on_drag: Rc::new(|_, _, _, _, _| {}),
            on_close: Rc::new(move |window, cx| { if let Some(view) = weak.upgrade() { view.update(cx, |view, cx| dismiss(view, window, cx)); } }),
            on_outside: None, is_dragging: false, width: None, max_height: super::available_size(window, cx).height.as_f32().min(crate::ui::scale::logical(480.0)),
            header: None, on_item_hover: None, on_width_calculated: None, on_hover_change: None,
        }.into_any_element()
    }, move |view, window, cx| close_menu(view, window, cx));
}
