use super::super::{ContextMenuContent, ContextMenuInstance};
use super::{parse_advanced_path, AdvancedSearchDialog, AdvancedSearchWindow, OxydeApp};
use crate::ui::{
    components::input::TextInput,
    i18n::{I18nContext, Message},
};
use gpui::*;
use oxyde_lib::commands::search::AdvancedSearchOptions;

pub(crate) fn close_advanced_search_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<AdvancedSearchWindow>() {
            let _ = handle.update(cx, |_, window, cx| { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() });
        }
    }
}

impl OxydeApp {
    pub(crate) fn open_advanced_search(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.context_menus.clear();
        self.settings_open = false;
        for handle in cx.windows() {
            if let Some(handle) = handle.downcast::<AdvancedSearchWindow>() {
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                });
                cx.notify();
                return;
            }
        }
        let pane = self.state.active_pane_id.clone();
        let Some(tab) = self.state.find_pane(&pane).map(|p| p.active_tab()) else {
            return;
        };
        let (root, old) = super::super::search::parse_search_path(&tab.path)
            .unwrap_or((tab.path.clone(), String::new()));
        let options = parse_advanced_path(&tab.path)
            .map(|(_, options)| options)
            .unwrap_or(AdvancedSearchOptions {
                query: old,
                recursive: true,
                ..Default::default()
            });
        let owner = cx.entity().downgrade();
        let title = cx.i18n().text(Message::ToolSearch);
        // A new window draws synchronously; defer until the owner update ends.
        cx.defer(move |cx| {
            if owner.upgrade().is_none() {
                return;
            }
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(160.0), px(140.0)),
                        size(px(760.0), px(520.0)),
                    ))),
                    is_resizable: false,
                    is_minimizable: false,
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some(title),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| AdvancedSearchWindow::new(owner, pane, root, options, window, cx))
                },
            );
            if let Err(error) = result {
                log::error!("Cannot open advanced search window: {error}");
            }
        });
        cx.notify();
    }
}

impl AdvancedSearchWindow {
    pub(super) fn new(
        owner: WeakEntity<OxydeApp>,
        pane: String,
        root: String,
        options: AdvancedSearchOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let unit = if options
            .min_size
            .into_iter()
            .chain(options.max_size)
            .any(|v| v % 1024 != 0)
        {
            0
        } else {
            1
        };
        let divisor = 1024u64.pow(unit as u32);
        let date = |v: Option<u64>| {
            v.and_then(|v| chrono::DateTime::from_timestamp_millis(v as i64))
                .map(|v| {
                    v.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d")
                        .to_string()
                })
                .unwrap_or_default()
        };
        let values = vec![
            options.query.clone(),
            root,
            options.content.clone(),
            options
                .min_size
                .map(|v| (v / divisor).to_string())
                .unwrap_or_default(),
            options
                .max_size
                .map(|v| (v / divisor).to_string())
                .unwrap_or_default(),
            date(options.min_date),
            date(options.max_date),
        ];
        let mut inputs = Vec::new();
        for (index, value) in values.iter().enumerate() {
            let change = cx.entity().downgrade();
            let enter = cx.entity().downgrade();
            let escape = cx.entity().downgrade();
            let menu = cx.entity().downgrade();
            inputs.push(cx.new(|cx| {
                TextInput::new(
                    cx,
                    "",
                    value.clone(),
                    move |value, _, cx| {
                        let _ = change.update(cx, |this, cx| {
                            if let Some(d) = &mut this.advanced_search {
                                d.values[index] = value.into();
                                d.error = false;
                            }
                            cx.notify();
                        });
                    },
                    move |window, cx| {
                        let _ =
                            enter.update(cx, |this, cx| this.submit_advanced_search(window, cx));
                    },
                    move |window, cx| {
                        let _ =
                            escape.update(cx, |this, cx| this.close_advanced_search(window, cx));
                    },
                    move |input, selected, position, window, cx| {
                        let _ = menu.update(cx, |this, cx| {
                            this.open_text_context_menu(input, selected, position, window, cx)
                        });
                    },
                    false,
                    true,
                    false,
                    None,
                    window,
                )
            }));
        }
        let focus = inputs[0].read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        let advanced_search = Some(AdvancedSearchDialog {
            inputs,
            values,
            flags: [
                options.regex,
                options.content_regex,
                options.recursive,
                options.archives,
                options.case_sensitive,
                options.ignore_accents,
            ],
            unit,
            help: false,
            error: false,
            focus: cx.focus_handle(),
            pane,
            calendar: None,
            controls: (0..14).map(|_| cx.focus_handle()).collect(),
            help_scroll: ScrollHandle::new(),
            help_metrics: (0.0, 0.0),
            help_drag: None,
        });
        window.on_window_should_close(cx, |window, cx| { crate::ui::components::tool_window::close_transients(window, cx); true });
        crate::ui::components::tool_window::install_icon("icons/search.svg", window);
        let mut subscriptions = Vec::new();
        if let Some(main) = owner.upgrade() {
            subscriptions.push(
                cx.observe_release_in(&main, window, |_, _, window, cx| { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() }),
            );
        }
        Self {
            owner,
            advanced_search,
            context_menus: Vec::new(),
            handle: cx.entity().downgrade(),
            scrollbar_origins: std::collections::HashMap::new(),
            grab_offset: 0.0,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn open_text_context_menu(
        &mut self,
        input: Entity<TextInput>,
        has_selection: bool,
        position: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::ui::components::dropdown::{DropdownItem, IconName};

        let Some(handle) = self.handle.upgrade() else {
            return;
        };
        let can_paste = cx
            .read_from_clipboard()
            .and_then(|clipboard| clipboard.text())
            .is_some();

        let mut items = Vec::new();
        for (label, command, is_dimmed) in [
            (Message::Cut, 0u8, !has_selection),
            (Message::Copy, 1u8, !has_selection),
            (Message::Paste, 2u8, !can_paste),
            (Message::SelectAll, 3u8, false),
        ] {
            if command == 3 {
                items.push(DropdownItem::separator());
            }
            let h = handle.downgrade();
            let target_input = input.clone();
            let mut item =
                DropdownItem::new(cx.i18n().text(label).to_string(), move |window, app| {
                    target_input.update(app, |input, cx| match command {
                        0 => input.cut_selected_text(window, cx),
                        1 => input.copy_selected_text(cx),
                        2 => input.paste_from_clipboard(window, cx),
                        _ => input.select_all_text(cx),
                    });
                    let _ = h.update(app, |_this, cx| {
                        cx.notify();
                    });
                });
            item.icon = match command {
                0 => Some(IconName::Scissors),
                1 => Some(IconName::Copy),
                2 => Some(IconName::ClipboardPaste),
                3 => Some(IconName::SquareDashedMousePointer),
                _ => None,
            };
            item.is_dimmed = is_dimmed;
            items.push(item);
        }

        self.context_menus.clear();
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position,
            content: ContextMenuContent::Items(items, None),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 0.0,
            open_left: false,
            hovered_item: None,
            native_request_id: None,
        });
        cx.notify();
    }

    pub(super) fn close_advanced_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.context_menus.is_empty() {
            self.context_menus.clear();
        } else if self
            .advanced_search
            .as_mut()
            .is_some_and(|d| d.calendar.take().is_some())
        {
            // Escape dismisses the date picker before closing the tool window.
        } else {
            { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() };
        }
        cx.notify();
    }
}
