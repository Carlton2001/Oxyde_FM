use crate::ui::components::input::TextInputOptions;
use crate::ui::app::{ContextMenuContent, ContextMenuHeader, ContextMenuInstance, OxydeApp};
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;

impl OxydeApp {
    pub fn update_filter_size(
        &mut self,
        pane_id: &str,
        category: crate::ui::workspace::SizeCategory,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if active {
                tab.filter_sizes.insert(category);
            } else {
                tab.filter_sizes.remove(&category);
            }
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn open_type_filter_menu(
        &mut self,
        pane_id: &str,
        pos: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane(pane_id) {
            let tab = pane.active_tab();
            let tab_files = tab.files.clone();
            let current_filters = tab.filter_types.clone();
            let items = crate::ui::components::file_list::get_type_filter_items(
                cx.i18n(),
                self.handle.as_ref().unwrap().clone(),
                pane_id.to_string(),
                tab_files,
                current_filters,
                pos,
            );

            self.context_menus.clear();
            self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                position: pos,
                content: ContextMenuContent::Items(
                    items,
                    Some(ContextMenuHeader::TypeSelectAll {
                        pane_id: pane_id.to_string(),
                    }),
                ),
                scroll_handle: UniformListScrollHandle::new(),
                width: px(0.0),
                bottom_padding: 3.0,
                open_left: false,
                hovered_item: None,
                native_request_id: None,
            });
            cx.notify();
        }
    }

    pub fn open_size_filter_menu(
        &mut self,
        pane_id: &str,
        pos: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane(pane_id) {
            let tab = pane.active_tab();
            let tab_files = tab.files.clone();
            let current_filters = tab.filter_sizes.clone();
            let items = crate::ui::components::file_list::get_size_filter_items(
                cx.i18n(),
                self.handle.as_ref().unwrap().clone(),
                pane_id.to_string(),
                tab_files,
                current_filters,
                pos,
            );

            self.context_menus.clear();
            self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                position: pos,
                content: ContextMenuContent::Items(
                    items,
                    Some(ContextMenuHeader::SizeSelectAll {
                        pane_id: pane_id.to_string(),
                    }),
                ),
                scroll_handle: UniformListScrollHandle::new(),
                width: px(0.0),
                bottom_padding: 3.0,
                open_left: false,
                hovered_item: None,
                native_request_id: None,
            });
            cx.notify();
        }
    }

    pub fn update_filter_date(
        &mut self,
        pane_id: &str,
        category: crate::ui::date_groups::DateGroup,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if active {
                tab.filter_dates.insert(category);
            } else {
                tab.filter_dates.remove(&category);
            }
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn open_date_filter_menu(
        &mut self,
        pane_id: &str,
        pos: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane(pane_id) {
            let tab = pane.active_tab();
            let tab_files = tab.files.clone();
            let current_filters = tab.filter_dates.clone();
            let items = crate::ui::components::file_list::get_date_filter_items(
                cx.i18n(),
                self.handle.as_ref().unwrap().clone(),
                pane_id.to_string(),
                tab_files,
                current_filters,
                pos,
            );

            self.context_menus.clear();
            self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                position: pos,
                content: ContextMenuContent::Items(
                    items,
                    Some(ContextMenuHeader::DateSelectAll {
                        pane_id: pane_id.to_string(),
                    }),
                ),
                scroll_handle: UniformListScrollHandle::new(),
                width: px(0.0),
                bottom_padding: 3.0,
                open_left: false,
                hovered_item: None,
                native_request_id: None,
            });
            cx.notify();
        }
    }

    pub fn toggle_sort(
        &mut self,
        pane_id: &str,
        column: crate::ui::workspace::SortColumn,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if tab.sort_column == column {
                tab.sort_direction = match tab.sort_direction {
                    crate::ui::workspace::SortDirection::Ascending => {
                        crate::ui::workspace::SortDirection::Descending
                    }
                    crate::ui::workspace::SortDirection::Descending => {
                        crate::ui::workspace::SortDirection::Ascending
                    }
                };
            } else {
                tab.sort_column = column;
                tab.sort_direction = crate::ui::workspace::SortDirection::Ascending;
            }
            tab.sort_files();
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn toggle_filter_type(&mut self, pane_id: &str, filter: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if tab.filter_types.contains(&filter) {
                tab.filter_types.remove(&filter);
            } else {
                tab.filter_types.insert(filter);
            }
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn clear_filter_types(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_types.clear();
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn clear_filter_sizes(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_sizes.clear();
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn clear_filter_dates(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_dates.clear();
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn update_filter_name(&mut self, pane_id: &str, name: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_name = name;
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn open_name_filter(
        &mut self,
        pane_id: &str,
        pos: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let h_cap = self.handle.as_ref().unwrap().clone();
        let pid_cap = pane_id.to_string();

        let initial_text = self
            .state
            .find_pane(pane_id)
            .map(|p: &crate::ui::workspace::PaneState| p.active_tab().filter_name.clone())
            .unwrap_or_default();

        let input = cx.new(|cx| {
            let h_change = h_cap.clone();
            let pid_change = pid_cap.clone();
            let h_enter = h_cap.clone();
            let h_esc = h_cap.clone();
            let pid_esc = pid_cap.clone();
            let h_context = h_cap.clone();

            crate::ui::components::input::TextInput::new(
                cx,
                cx.i18n().text(Message::NameFilterPlaceholder).to_string(),
                initial_text,
                move |text, _, cx| {
                    h_change.update(cx, |this, cx| {
                        this.update_filter_name(&pid_change, text.to_string(), cx)
                    });
                },
                move |_, cx| {
                    h_enter.update(cx, |this, _| {
                        this.context_menus.clear();
                    });
                },
                move |_, cx| {
                    h_esc.update(cx, |this, cx| {
                        this.update_filter_name(&pid_esc, "".to_string(), cx);
                        this.context_menus.clear();
                    });
                },
                move |input, has_selection, position, window, cx| {
                    h_context.update(cx, |this, cx| {
                        this.open_text_context_menu(input, has_selection, position, window, cx)
                    });
                },
                TextInputOptions {
                    clearable: true,
                    ..Default::default()
                },
                window,
            )
        });

        self.context_menus.clear();
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position: pos,
            content: ContextMenuContent::NameFilter {
                input: input.clone(),
            },
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 2.0,
            open_left: false,
            hovered_item: None,
            native_request_id: None,
        });
        cx.notify();

        #[cfg(not(target_os = "windows"))]
        let focus_handle = input.read(cx).focus_handle.clone();
        #[cfg(not(target_os = "windows"))]
        cx.on_next_frame(window, move |_, window, cx| {
            window.focus(&focus_handle, cx);
        });
    }
    pub fn update_filter_original_location(&mut self, pane_id: &str, name: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_original_location = name;
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn open_original_location_filter(
        &mut self,
        pane_id: &str,
        pos: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let h_cap = self.handle.as_ref().unwrap().clone();
        let pid_cap = pane_id.to_string();

        let initial_text = self
            .state
            .find_pane(pane_id)
            .map(|p: &crate::ui::workspace::PaneState| p.active_tab().filter_original_location.clone())
            .unwrap_or_default();

        let input = cx.new(|cx| {
            let h_change = h_cap.clone();
            let pid_change = pid_cap.clone();
            let h_enter = h_cap.clone();
            let h_esc = h_cap.clone();
            let pid_esc = pid_cap.clone();
            let h_context = h_cap.clone();

            crate::ui::components::input::TextInput::new(
                cx,
                cx.i18n().text(Message::OriginalLocationFilterPlaceholder).to_string(),
                initial_text,
                move |text, _, cx| {
                    h_change.update(cx, |this, cx| {
                        this.update_filter_original_location(&pid_change, text.to_string(), cx)
                    });
                },
                move |_, cx| {
                    h_enter.update(cx, |this, _| {
                        this.context_menus.clear();
                    });
                },
                move |_, cx| {
                    h_esc.update(cx, |this, cx| {
                        this.update_filter_original_location(&pid_esc, "".to_string(), cx);
                        this.context_menus.clear();
                    });
                },
                move |input, has_selection, position, window, cx| {
                    h_context.update(cx, |this, cx| {
                        this.open_text_context_menu(input, has_selection, position, window, cx)
                    });
                },
                TextInputOptions {
                    clearable: true,
                    ..Default::default()
                },
                window,
            )
        });

        self.context_menus.clear();
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position: pos,
            content: ContextMenuContent::NameFilter {
                input: input.clone(),
            },
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 2.0,
            open_left: false,
            hovered_item: None,
            native_request_id: None,
        });
        cx.notify();

        #[cfg(not(target_os = "windows"))]
        let focus_handle = input.read(cx).focus_handle.clone();
        #[cfg(not(target_os = "windows"))]
        cx.on_next_frame(window, move |_, window, cx| {
            window.focus(&focus_handle, cx);
        });
    }
}
