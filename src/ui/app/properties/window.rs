use super::{PropertiesDialog, PropertiesWindow};
use crate::ui::app::OxydeApp;
use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
use crate::ui::components::input::TextInput;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub(crate) fn close_properties_windows(cx: &mut App) {
    for window in cx.windows() {
        if let Some(window) = window.downcast::<PropertiesWindow>() {
            let _ = window.update(cx, |_, window, cx| { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() });
        }
    }
}

impl OxydeApp {
    pub(crate) fn open_file_properties(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_properties_for_paths(vec![path], window, cx);
    }

    pub(crate) fn open_properties_for_paths(
        &mut self,
        mut paths: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        paths.sort();
        paths.dedup();
        let Some(path) = paths.first().cloned() else { return; };
        self.context_menus.clear();
        if paths.len() == 1 && path == "trash://" {
            self.open_trash_settings(window, cx);
            return;
        }
        if paths.len() == 1 && path == "__network_vincinity__" {
            cx.spawn(async move |_, _| {
                let _ = tokio::task::spawn_blocking(move || {
                    oxyde_lib::platform::platform().show_properties_dialog(&path)
                })
                .await;
            })
            .detach();
            return;
        }
        let owner = cx.entity().downgrade();
        let drive = self
            .state
            .drives
            .iter()
            .find(|drive| crate::ui::components::sidebar::sidebar_paths_equal(&drive.path, &path))
            .cloned();
        let title = cx.i18n().text(Message::Properties);
        let origin = window.bounds().origin + point(px(48.0), px(48.0));
        let native_owner = raw_window_handle::HasWindowHandle::window_handle(window)
            .ok().map(|handle| handle.as_raw());
        cx.defer(move |cx| {
            if owner.upgrade().is_none() {
                return;
            }
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        origin,
                        size(px(480.0), px(600.0)),
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
                    window.on_window_should_close(cx, |window, cx| { crate::ui::components::tool_window::close_transients(window, cx); true });
                    if let (Some(owner), Ok(handle)) = (
                        native_owner, raw_window_handle::HasWindowHandle::window_handle(window),
                    ) {
                        if let Err(error) = oxyde_lib::platform::platform().set_window_owner(handle.as_raw(), owner) {
                            log::warn!("Cannot attach properties window to Oxyde: {error}");
                        }
                    }
                    cx.new(|cx| {
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        crate::ui::components::tool_window::install_icon(
                            "icons/settings-2.svg",
                            window,
                        );
                        let mut subscriptions = Vec::new();
                        if let Some(main) = owner.upgrade() {
                            subscriptions.push(cx.observe_release_in(
                                &main,
                                window,
                                |_, _, window, cx| { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() },
                            ));
                        }
                        let mut view = PropertiesWindow {
                            owner,
                            properties_dialog: Some(PropertiesDialog {
                                path: path.clone(),
                                paths: paths.clone(),
                                selection: None,
                                system_properties_pending: false,
                                system_properties_error: None,
                                revision: 0,
                                properties: None,
                                drive,
                                calculating: false,
                                error: None,
                                focus,
                                ok_focus: cx.focus_handle(),
                                shortcut_tab: false,
                                shortcut_inputs: Vec::new(),
                                run_window: 1,
                                run_menu: None,
                                run_focus: cx.focus_handle(),
                                run_bounds: None,
                                saving: false,
                                validation_error: None,
                            }),
                            context_menus: Vec::new(),
                            scrollbar_origins: Default::default(),
                            _subscriptions: subscriptions,
                            _calculation: None,
                            cancellation: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                                false,
                            )),
                        };
                        if paths.len() > 1 {
                            view.load_selection(paths, cx);
                        } else {
                            view.load(path, cx);
                        }
                        view
                    })
                },
            );
            if let Err(error) = result {
                log::error!("Cannot open properties window: {error}");
            }
        });
        cx.notify();
    }
}

impl PropertiesWindow {
    pub(super) fn use_system_icons(&self, cx: &App) -> bool {
        self.owner
            .upgrade()
            .is_some_and(|owner| owner.read(cx).use_system_icons())
    }
}

impl Render for PropertiesWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        window.set_window_title(cx.i18n().text(Message::Properties).as_ref());
        if let Some(owner) = self.owner.upgrade() {
            window.set_rem_size(px(owner.read(cx).ui_rem_size() as f32));
        }
        let handle = cx.entity();
        let mut root = div()
            .size_full()
            .relative()
            .bg(theme.active_background)
            .text_color(theme.text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25));
        if self.properties_dialog.is_some() {
            root = root.child(super::dialog::render(self, handle.clone(), window, cx));
        }
        for (index, menu) in self.context_menus.iter().enumerate() {
            let crate::ui::app::ContextMenuContent::Items(items, _) = &menu.content else {
                continue;
            };
            let close = handle.clone();
            #[cfg(target_os = "windows")]
            if crate::ui::components::popup::supported(window, cx) {
                let scroll = menu.scroll_handle.clone();
                crate::ui::components::popup::text_menu(
                    format!("properties-text-menu-{index}"), crate::ui::components::popup::PopupAnchor::point(menu.position, false),
                    close.clone(), handle.clone(), items.clone(), scroll.clone(), window, cx,
                    move |view| view.context_menus.get(index).is_some_and(|menu| std::rc::Rc::ptr_eq(&menu.scroll_handle.0, &scroll.0)),
                    |view, _, cx| { view.context_menus.clear(); cx.notify(); },
                );
                continue;
            }
                        {
            root = root.child(deferred(
                anchored().position(menu.position).snap_to_window().child(
                    crate::ui::components::dropdown::GenericDropdown {
                        id: ("properties-menu", index).into(),
                        items: items.clone(),
                        reserve_icon_space: true,
                        is_context_menu: true,
                        bottom_padding: 4.0,
                        scroll_handle: menu.scroll_handle.clone(),
                        h_app: handle.clone(),
                        on_drag: std::rc::Rc::new(|_, _, _, _, _| {}),
                        on_close: std::rc::Rc::new(move |_, cx| {
                            close.update(cx, |this, cx| {
                                this.context_menus.clear();
                                cx.notify();
                            })
                        }),
                        on_outside: None,
                        is_dragging: false,
                        width: None,
                        max_height: window.viewport_size().height.as_f32(),
                        header: None,
                        on_item_hover: None,
                        on_width_calculated: None,
                        on_hover_change: None,
                    },
                ),
            ));
            }
        }
        root
    }
}

impl PropertiesWindow {
    pub(crate) fn open_text_context_menu(
        &mut self,
        input: Entity<TextInput>,
        has_selection: bool,
        position: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::ui::components::dropdown::{DropdownItem, IconName};

        let handle = cx.entity();
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
}
