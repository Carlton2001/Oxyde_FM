//! Sort by and Filter by of the grid view's context menu. Both submenus, and the filter panel
//! opened from the second, stay open while the pointer goes elsewhere: a click outside the
//! menus closes them.
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::dropdown::DropdownItem;
use crate::ui::i18n::I18nContext;
use crate::ui::workspace::FilterColumn;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GridMenuKind {
    Sort,
    Filters,
}

#[derive(Clone)]
pub(crate) struct GridMenu {
    pub kind: GridMenuKind,
    pub pane_id: String,
    pub items: Vec<DropdownItem>,
}

impl OxydeApp {
    /// Whether menu `index` stays open while the pointer leaves it and its parent.
    pub(crate) fn menu_stays_open(&self, index: usize) -> bool {
        index > 0
            && self.context_menus.get(index).is_some_and(|menu| {
                matches!(menu.content, ContextMenuContent::Grid(_) | ContextMenuContent::ColumnFilter(_))
            })
    }

    /// Opens Sort by or Filter by beside the root menu's row, unless it is already open.
    pub(crate) fn open_grid_menu(&mut self, kind: GridMenuKind, pane_id: &str, position: Point<Pixels>, open_left: bool, cx: &mut Context<Self>) {
        let Some(handle) = self.handle.clone() else { return };
        if self.context_menus.get(1).is_some_and(|menu| matches!(&menu.content, ContextMenuContent::Grid(open) if open.kind == kind && open.pane_id == pane_id)) {
            return;
        }
        let items = crate::ui::components::grid_menu_items(kind, self, handle, pane_id, cx.i18n());
        self.context_menus.truncate(1);
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position,
            content: ContextMenuContent::Grid(GridMenu { kind, pane_id: pane_id.to_string(), items }),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            open_left,
            hovered_item: None,
            native_request_id: None,
            open_up: false,
        });
        cx.notify();
    }

    /// Rebuilds the open grid submenus from the tab, after a sort or filter change.
    pub(crate) fn refresh_grid_menus(&mut self, cx: &mut Context<Self>) {
        let Some(handle) = self.handle.clone() else { return };
        let rebuilt: Vec<_> = self.context_menus.iter().enumerate().filter_map(|(index, menu)| match &menu.content {
            ContextMenuContent::Grid(grid) => Some((index, crate::ui::components::grid_menu_items(grid.kind, self, handle.clone(), &grid.pane_id, cx.i18n()))),
            _ => None,
        }).collect();
        for (index, items) in rebuilt {
            if let Some(ContextMenuContent::Grid(grid)) = self.context_menus.get_mut(index).map(|menu| &mut menu.content) {
                grid.items = items;
            }
        }
        cx.notify();
    }

    /// Opens the filter panel of `column` beside Filter by's row. Its fields need the main
    /// window, which the hover is being delivered in: the opening waits for it to be free.
    pub(crate) fn open_filter_submenu(&mut self, pane_id: &str, column: FilterColumn, position: Point<Pixels>, open_left: bool, cx: &mut Context<Self>) {
        let (Some(window), Some(handle)) = (self.main_window, self.handle.clone()) else { return };
        let pane_id = pane_id.to_string();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| handle.update(cx, |this, cx| {
                let Some(parent) = this.context_menus.iter().position(|menu| matches!(&menu.content, ContextMenuContent::Grid(grid) if grid.kind == GridMenuKind::Filters && grid.pane_id == pane_id)) else { return };
                if this.context_menus.get(parent + 1).is_some_and(|menu| matches!(&menu.content, ContextMenuContent::ColumnFilter(panel) if panel.column == column && panel.pane_id == pane_id)) {
                    return;
                }
                let Some(panel) = this.column_filter_panel(&pane_id, column, window, cx) else { return };
                this.context_menus.truncate(parent + 1);
                this.context_menus.push(ContextMenuInstance {
                    native_bounds: None,
                    position,
                    content: ContextMenuContent::ColumnFilter(Box::new(panel)),
                    scroll_handle: UniformListScrollHandle::new(),
                    width: px(0.0),
                    open_left,
                    hovered_item: None,
                    native_request_id: None,
                    open_up: false,
                });
                cx.notify();
            }));
        });
    }
}
