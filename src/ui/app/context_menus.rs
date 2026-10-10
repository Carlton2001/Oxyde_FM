use super::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::input::TextInput;
use crate::ui::i18n::{FluentArgs, I18n, I18nContext, Message};
use crate::ui::scale::logical;
use gpui::*;

pub(super) fn native_menu_max_height(viewport_height: Pixels) -> f32 {
    // Leave 8 px on each side, in addition to the menu's 6 px of chrome.
    (viewport_height.as_f32() - logical(22.0)).max(logical(26.0))
}

pub(super) fn context_menu_contains_point(
    menu: &ContextMenuInstance,
    position: Point<Pixels>,
    viewport_height: Pixels,
) -> bool {
    let height = context_menu_height(menu, viewport_height);
    let left = if menu.open_left {
        menu.position.x - menu.width
    } else {
        menu.position.x
    };
    let top = context_menu_visible_top(menu.position.y, height, viewport_height);
    Bounds::new(point(left, top), size(menu.width, height)).contains(&position)
}

pub(super) fn context_menu_height(menu: &ContextMenuInstance, viewport_height: Pixels) -> Pixels {
    let (item_count, header_height, max_height) = match &menu.content {
        ContextMenuContent::Items(items, header) => (
            items.iter().filter(|item| !matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count(),
            if header.is_some() { logical(42.0) } else { logical(6.0) },
            logical(480.0),
        ),
        ContextMenuContent::NativeItems { items, .. } => {
            (items.len(), logical(6.0), native_menu_max_height(viewport_height))
        }
        ContextMenuContent::NameFilter { .. } => (0, crate::ui::style::row_height() + logical(8.0), logical(480.0)),
    };
    let header = matches!(&menu.content, ContextMenuContent::Items(_, Some(_)));
    let max_height = crate::ui::style::menu_list_max_height(viewport_height.as_f32(), header, menu.bottom_padding, max_height);
    let separator_height = match &menu.content {
        ContextMenuContent::Items(items, _) => items.iter().filter(|item| matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count() as f32 * logical(9.0),
        _ => 0.0,
    };
    px((item_count as f32 * logical(26.0) + separator_height).min(max_height) + header_height + logical(menu.bottom_padding))
}

pub(super) fn context_menu_visible_top(
    requested_top: Pixels,
    menu_height: Pixels,
    viewport_height: Pixels,
) -> Pixels {
    requested_top
        .min((viewport_height - menu_height).max(px(0.0)))
        .max(px(0.0))
}

impl OxydeApp {
    pub(crate) fn request_native_context_menu(
        &mut self,
        parent_menu_idx: usize,
        path: String,
        is_background: bool,
        position: Point<Pixels>,
        open_left: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        if parent_menu_idx >= self.context_menus.len() {
            return;
        }
        if let Some(child) = self.context_menus.get(parent_menu_idx + 1) {
            if matches!(
                &child.content,
                ContextMenuContent::NativeItems {
                    path: child_path,
                    is_background: child_background,
                    ..
                } if child_path == &path && *child_background == is_background
            ) && (child.native_request_id.is_some()
                || matches!(&child.content, ContextMenuContent::NativeItems { items, .. } if !items.is_empty()))
            {
                return;
            }
        }

        self.next_native_menu_request = self.next_native_menu_request.wrapping_add(1);
        let request_id = self.next_native_menu_request;
        self.context_menus.truncate(parent_menu_idx + 1);
        self.context_menus.push(ContextMenuInstance {
            position,
            content: ContextMenuContent::NativeItems {
                path: path.clone(),
                is_background,
                items: vec![oxyde_lib::models::WinMenuItem {
                    id: -1,
                    label: cx.i18n().text(Message::NativeMenuLoading).to_string(),
                    verb: None,
                    has_submenu: false,
                    children: Vec::new(),
                }],
            },
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 0.0,
            open_left,
            hovered_item: None,
            native_request_id: Some(request_id),
        });
        cx.notify();

        let language = cx.i18n().language();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let path_for_worker = path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    oxyde_lib::platform::platform()
                        .native_context_menu_items(&path_for_worker, is_background)
                })
                .await;
                let i18n = I18n::for_language(language);
                let result = match result {
                    Ok(Ok(items)) if !items.is_empty() => Ok(items),
                    Ok(Ok(_)) => Err(i18n.text(Message::NativeMenuEmpty).to_string()),
                    Ok(Err(error)) => {
                        let mut args = FluentArgs::new();
                        args.set("detail", error.to_string());
                        Err(i18n
                            .text_with_args(Message::NativeMenuFailed, &args)
                            .to_string())
                    }
                    Err(error) => {
                        let mut args = FluentArgs::new();
                        args.set("detail", error.to_string());
                        Err(i18n
                            .text_with_args(Message::NativeMenuFailed, &args)
                            .to_string())
                    }
                };
                handle.update(&mut cx, move |this, cx| {
                    let Some(menu) = this.context_menus.get_mut(parent_menu_idx + 1) else {
                        return;
                    };
                    if menu.native_request_id != Some(request_id) {
                        return;
                    }
                    let ContextMenuContent::NativeItems {
                        path: current_path,
                        is_background: current_background,
                        items,
                    } = &mut menu.content
                    else {
                        return;
                    };
                    if current_path != &path || *current_background != is_background {
                        return;
                    }
                    match result {
                        Ok(native_items) => *items = native_items,
                        Err(message) => {
                            *items = vec![oxyde_lib::models::WinMenuItem {
                                id: -1,
                                label: message,
                                verb: None,
                                has_submenu: false,
                                children: Vec::new(),
                            }];
                        }
                    }
                    menu.native_request_id = None;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn execute_native_context_menu_item(
        &mut self,
        path: String,
        id: i32,
        is_background: bool,
        creates_item: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        let tab_id = self
            .state
            .layout
            .find_pane(&self.state.active_pane_id)
            .map(|pane| pane.active_tab().id.clone());
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    let before = if creates_item {
                        std::fs::read_dir(&path).ok().map(|entries| {
                            entries
                                .filter_map(Result::ok)
                                .map(|entry| entry.path())
                                .collect::<std::collections::HashSet<_>>()
                        })
                    } else {
                        None
                    };
                    oxyde_lib::platform::platform().execute_native_context_menu_item(
                        &path,
                        id,
                        is_background,
                    )?;
                    if let Some(before) = before {
                        for _ in 0..20 {
                            let added: Vec<_> = std::fs::read_dir(&path)
                                .map_err(|error| error.to_string())?
                                .filter_map(Result::ok)
                                .filter(|entry| !before.contains(&entry.path()))
                                .collect();
                            if added.len() == 1 {
                                let entry = &added[0];
                                return Ok(Some((
                                    entry.path().to_string_lossy().into_owned(),
                                    entry
                                        .file_type()
                                        .map_err(|error| error.to_string())?
                                        .is_dir(),
                                )));
                            }
                            if added.len() > 1 {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                    Ok::<_, String>(None)
                })
                .await;
                match result {
                    Ok(Ok(Some((path, is_dir)))) => {
                        handle.update(&mut cx, |this, cx| {
                            if let Some(tab_id) = tab_id {
                                this.queue_created_item_rename(tab_id, path, is_dir, cx);
                            }
                        });
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => log::warn!("Native context-menu action failed: {error:?}"),
                    Err(error) => log::warn!("Native context-menu worker failed: {error}"),
                }
                handle.update(&mut cx, |_, cx| cx.notify());
            }
        })
        .detach();
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

        let Some(handle) = self.handle.clone() else {
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
            let h = handle.clone();
            let target_input = input.clone();
            let mut item =
                DropdownItem::new(cx.i18n().text(label).to_string(), move |window, app| {
                    target_input.update(app, |input, cx| match command {
                        0 => input.cut_selected_text(window, cx),
                        1 => input.copy_selected_text(cx),
                        2 => input.paste_from_clipboard(window, cx),
                        _ => input.select_all_text(cx),
                    });
                    h.update(app, |_this, cx| {
                        cx.notify();
                    });
                });
            item.icon = match command {
                1 => Some(IconName::Copy),
                3 => Some(IconName::ALargeSmall),
                _ => None,
            };
            item.is_dimmed = is_dimmed;
            items.push(item);
        }

        self.context_menus.clear();
        self.context_menus.push(ContextMenuInstance {
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
