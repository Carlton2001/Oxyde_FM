use super::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::date_groups::GroupMode;
use crate::ui::i18n::I18nContext;
use gpui::*;

impl OxydeApp {
    pub(crate) fn set_grouping(&mut self, pane_id: &str, mode: GroupMode, cx: &mut Context<Self>) {
        self.finish_pointer_drag(cx);
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if !tab.supports_grouping(mode) {
                return;
            }
            tab.group_mode = mode;
            tab.list_scroll
                .0
                .borrow()
                .base_handle
                .set_offset(point(px(0.0), px(0.0)));
            tab.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        }
        self.context_menus.clear();
        self.save_workspace(cx);
        cx.notify();
    }

    pub(crate) fn open_grouping_menu(
        &mut self,
        pane_id: &str,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.state.find_pane(pane_id) else {
            return;
        };
        let selected = pane.active_tab().grouping();
        let handle = cx.entity();
        let mut choices = Vec::new();
        let mut modes = pane.active_tab().available_groupings();
        modes.sort_by_cached_key(|mode| (
            *mode != GroupMode::None,
            cx.i18n().text(mode.message()).to_lowercase(),
        ));
        for mode in modes {
            let label = mode.message();
            let handle = handle.clone();
            let pane_id = pane_id.to_string();
            let mut item = DropdownItem::new(cx.i18n().text(label).to_string(), move |_, cx| {
                handle.update(cx, |view, cx| view.set_grouping(&pane_id, mode, cx));
            });
            if mode == selected {
                item.icon = Some(IconName::Check);
            }
            choices.push(item);
        }
        self.context_menus.clear();
        self.context_menus
            .push(grouping_menu(position, false, choices));
        cx.notify();
    }
}

fn grouping_menu(
    position: Point<Pixels>,
    open_left: bool,
    items: Vec<DropdownItem>,
) -> ContextMenuInstance {
    ContextMenuInstance {
        native_bounds: None,
        position,
        content: ContextMenuContent::Items(items, None),
        scroll_handle: UniformListScrollHandle::new(),
        width: px(0.0),
        bottom_padding: 0.0,
        open_left,
        hovered_item: None,
        native_request_id: None,
        open_up: true,
    }
}
