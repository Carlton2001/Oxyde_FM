use super::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::input::TextInput;
use crate::ui::i18n::{FluentArgs, I18n, I18nContext, Message};
use crate::ui::scale::logical;
use gpui::*;

pub(super) const NATIVE_MENU_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Loads whose Shell call is still running, timed out or not.
const MAX_NATIVE_MENU_LOADS: usize = 4;
static NATIVE_MENU_LOADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Counts one native menu load until its Shell call returns.
pub(super) struct NativeMenuLoad;

impl NativeMenuLoad {
    pub(super) fn start() -> Option<Self> {
        use std::sync::atomic::Ordering;
        NATIVE_MENU_LOADS
            .try_update(Ordering::AcqRel, Ordering::Acquire, |loads| {
                (loads < MAX_NATIVE_MENU_LOADS).then_some(loads + 1)
            })
            .ok()
            .map(|_| Self)
    }
}

impl Drop for NativeMenuLoad {
    fn drop(&mut self) {
        NATIVE_MENU_LOADS.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

pub(super) fn native_menu_max_height(viewport_height: Pixels) -> f32 {
    // Leave 8 px on each side, in addition to the menu's 6 px of chrome.
    (viewport_height.as_f32() - logical(22.0)).max(crate::ui::style::menu_item_height())
}

pub(super) fn context_menu_contains_point(
    menu: &ContextMenuInstance,
    position: Point<Pixels>,
    viewport_height: Pixels,
) -> bool {
    if let Some(bounds) = menu.native_bounds { return bounds.contains(&position); }
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
    if let Some(bounds) = menu.native_bounds { return bounds.size.height; }
    if let ContextMenuContent::GridSize(_) = menu.content { return px(crate::ui::style::menu_item_height() + logical(10.0)); }
    let (item_count, header_height, max_height) = match &menu.content {
        ContextMenuContent::Items(items) | ContextMenuContent::Grid(super::filters::GridMenu { items, .. }) => (
            items.iter().filter(|item| !matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count(),
            logical(6.0),
            f32::INFINITY,
        ),
        ContextMenuContent::NativeItems { items, .. } => {
            (items.len(), logical(6.0), native_menu_max_height(viewport_height))
        }
        // Measured once drawn; until then, a header and a few rows.
        ContextMenuContent::ColumnFilter(_) => (6, logical(96.0), logical(320.0)),
        ContextMenuContent::GridSize(_) => (1, logical(6.0), f32::INFINITY),
    };
    let header = matches!(&menu.content, ContextMenuContent::ColumnFilter(_));
    let max_height = crate::ui::style::menu_list_max_height(viewport_height.as_f32(), header, max_height);
    let separator_height = match &menu.content {
        ContextMenuContent::Items(items) | ContextMenuContent::Grid(super::filters::GridMenu { items, .. }) => items.iter().filter(|item| matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count() as f32 * crate::ui::style::menu_separator_height(),
        _ => 0.0,
    };
    px((item_count as f32 * crate::ui::style::menu_item_height() + separator_height).min(max_height) + header_height + logical(4.0))
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

fn action_paths(
    clicked_path: &str,
    selection: Option<&std::collections::HashSet<String>>,
) -> Vec<String> {
    if let Some(selection) = selection.filter(|paths| paths.contains(clicked_path)) {
        let mut paths: Vec<_> = selection.iter().cloned().collect();
        paths.sort();
        paths
    } else {
        vec![clicked_path.to_owned()]
    }
}

#[cfg(test)]
#[path = "context_menus/tests/selection.rs"]
mod selection_tests;

fn is_open_with_submenu(item: &oxyde_lib::models::WinMenuItem) -> bool {
    if !item.has_submenu {
        return false;
    }
    // The scrape reads the popup's own "openas" verb in any language. The label,
    // in the OS language, remains for a handler whose popup carries no verb.
    let label = item.label.split('\t').next().unwrap_or_default().replace('&', "");
    let label = oxyde_lib::utils::remove_accents(&label.to_lowercase());
    matches!(
        label.trim().trim_end_matches(['.', '…']).trim_end(),
        "ouvrir avec" | "open with" | "offnen mit" | "abrir con" | "apri con"
    )
}

fn native_action_opens_media(
    items: &[oxyde_lib::models::WinMenuItem],
    id: i32,
    in_open: bool,
) -> bool {
    items.iter().any(|item| {
        let opens_media = in_open || is_open_with_submenu(item) || item.verb.as_deref().is_some_and(|verb| {
            ["open", "openas", "openwith", "play", "playwith"]
                .iter()
                .any(|opening| verb.eq_ignore_ascii_case(opening))
        });
        (item.id == id && !item.has_submenu && opens_media)
            || native_action_opens_media(&item.children, id, opens_media)
    })
}

#[cfg(test)]
#[path = "context_menus/tests/media_open.rs"]
mod media_open_tests;

impl OxydeApp {
    /// Closes the root menu when its own control is clicked again.
    pub(crate) fn close_menu_owned_by(&mut self, owner: &str, cx: &mut Context<Self>) -> bool {
        let owned = matches!((&self.context_menu_owner, self.context_menus.first()),
            (Some((key, handle)), Some(menu)) if key.as_ref() == owner && std::rc::Rc::ptr_eq(&handle.0, &menu.scroll_handle.0));
        if owned {
            self.context_menus.clear();
            self.context_menu_owner = None;
            cx.notify();
        }
        let dismissed = self.dismissed_menu_owner.take()
            .is_some_and(|(key, at)| key.as_ref() == owner && at.elapsed() < std::time::Duration::from_millis(300));
        owned || dismissed
    }

    /// Native menus close on the owner's button-down, before its control sees the click.
    pub(crate) fn note_menu_dismissed(&mut self, index: usize) {
        if index != 0 { return; }
        let owner = self.context_menu_owner.take();
        if let (Some((key, handle)), Some(menu)) = (owner, self.context_menus.first()) {
            if std::rc::Rc::ptr_eq(&handle.0, &menu.scroll_handle.0) {
                self.dismissed_menu_owner = Some((key, std::time::Instant::now()));
            }
        }
    }

    /// Records the control that opened the current root menu.
    pub(crate) fn mark_menu_owner(&mut self, owner: impl Into<SharedString>) {
        self.context_menu_owner = self.context_menus.first().map(|menu| (owner.into(), menu.scroll_handle.clone()));
    }

    pub(crate) fn context_action_paths(&self, clicked_path: &str) -> Vec<String> {
        action_paths(clicked_path, self.state.find_pane(&self.state.active_pane_id)
            .map(|pane| &pane.active_tab().selected_paths))
    }

    pub(crate) fn request_native_context_menu(
        &mut self,
        parent_menu_idx: usize,
        path: Vec<String>,
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
            native_bounds: None,
            position,
            content: ContextMenuContent::NativeItems {
                path: path.clone(),
                is_background,
                items: vec![oxyde_lib::models::WinMenuItem {
                    id: -1,
                    label: cx.i18n().text(Message::NativeMenuLoading).to_string(),
                    verb: None,
                    has_submenu: false,
                    is_folder_sharing_command: false,
                    children: Vec::new(),
                }],
            },
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            open_left,
            hovered_item: None,
            native_request_id: Some(request_id),
            open_up: false,
        });
        cx.notify();

        let language = cx.i18n().language();
        let timer = cx.background_executor().timer(NATIVE_MENU_TIMEOUT);
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                // A Shell extension can block its call forever, and the call cannot
                // be cancelled: the menu reports it, and stuck threads stay bounded.
                let load = NativeMenuLoad::start().map(|load| {
                    let path_for_worker = path.clone();
                    tokio::task::spawn_blocking(move || {
                        let _load = load;
                        oxyde_lib::platform::platform()
                            .native_context_menu_items(&path_for_worker, is_background)
                    })
                });
                let result = match load {
                    Some(task) => {
                        match futures::future::select(task, timer).await {
                            futures::future::Either::Left((result, _)) => Some(result),
                            futures::future::Either::Right(_) => None,
                        }
                    }
                    None => None,
                };
                let i18n = I18n::for_language(language);
                let result = match result {
                    None => Err(i18n.text(Message::NativeMenuTimeout).to_string()),
                    Some(Ok(Ok(items))) if !items.is_empty() => Ok(items),
                    Some(Ok(Ok(_))) => Err(i18n.text(Message::NativeMenuEmpty).to_string()),
                    Some(Ok(Err(error))) => {
                        let mut args = FluentArgs::new();
                        args.set("detail", crate::ui::format::error_detail(&error.to_string(), &i18n));
                        Err(i18n
                            .text_with_args(Message::NativeMenuFailed, &args)
                            .to_string())
                    }
                    Some(Err(error)) => {
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
                                is_folder_sharing_command: false,
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
        path: Vec<String>,
        item: oxyde_lib::models::WinMenuItem,
        is_background: bool,
        creates_item: bool,
        cx: &mut Context<Self>,
    ) {
        let opens_media = !is_background && self.context_menus.iter().any(|menu| {
            match &menu.content {
                ContextMenuContent::NativeItems { items, .. } => {
                    native_action_opens_media(items, item.id, false)
                }
                _ => false,
            }
        });
        self.context_menus.clear();
        if opens_media {
            for path in &path {
                self.pause_viewer_media_for_external_open(path, cx);
            }
        }
        let tab_id = if creates_item {
            self.state.layout.find_pane(&self.state.active_pane_id).map(|pane| pane.active_tab().id.clone())
        } else {
            None
        };
        self.run_native_menu_action(path, tab_id, item.is_folder_sharing_command, move |path| {
            oxyde_lib::platform::platform().execute_native_context_menu_item(path, &item, is_background)
        }, cx);
    }

    pub(super) fn run_native_menu_action(
        &mut self,
        path: Vec<String>,
        rename_tab: Option<String>,
        report_failure: bool,
        execute: impl FnOnce(&[String]) -> Result<(), String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else { return };
        let track_creation = rename_tab.is_some();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    let before = if track_creation {
                        std::fs::read_dir(&path[0]).ok().map(|entries| {
                            entries
                                .filter_map(Result::ok)
                                .map(|entry| entry.path())
                                .collect::<std::collections::HashSet<_>>()
                        })
                    } else {
                        None
                    };
                    // A file renamed or moved into the folder meanwhile (a finished
                    // download) keeps its older creation time; the Shell creates anew.
                    let invoked_at = std::time::SystemTime::now()
                        .checked_sub(std::time::Duration::from_secs(2))
                        .unwrap_or(std::time::UNIX_EPOCH);
                    execute(&path)?;
                    if let Some(before) = before {
                        for _ in 0..20 {
                            let added: Vec<_> = std::fs::read_dir(&path[0])
                                .map_err(|error| error.to_string())?
                                .filter_map(Result::ok)
                                .filter(|entry| !before.contains(&entry.path()))
                                .filter(|entry| {
                                    entry
                                        .metadata()
                                        .and_then(|metadata| metadata.created())
                                        .map_or(true, |created| created >= invoked_at)
                                })
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
                handle.update(&mut cx, |this, cx| {
                    match result.map_err(|error| error.to_string()).and_then(|result| result) {
                        Ok(Some((path, is_dir))) => {
                            if let Some(tab_id) = rename_tab {
                                this.queue_created_item_rename(tab_id, path, is_dir, cx);
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            log::warn!("Native context-menu action failed: {error}");
                            if report_failure {
                                this.toast(super::ToastKind::Error, Message::ToastFailed, error, 0, cx);
                            }
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn open_text_context_menu(
        &mut self,
        input: Entity<TextInput>,
        has_selection: bool,
        position: Point<Pixels>,
        window: &mut Window,
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
                0 => Some(IconName::Scissors),
                1 => Some(IconName::Copy),
                2 => Some(IconName::ClipboardPaste),
                3 => Some(IconName::SquareDashedMousePointer),
                _ => None,
            };
            item.is_dimmed = is_dimmed;
            items.push(item);
        }

        if window.root::<super::dialog_windows::DialogWindow>().flatten().is_some() {
            super::dialog_windows::show_text_menu(items, position, window, cx);
            return;
        }
        // Preserve the filter panel while the text menu of its field is open.
        if let Some(index) = self.context_menus.iter().position(|menu| matches!(menu.content, ContextMenuContent::ColumnFilter(_))) {
            self.context_menus.truncate(index + 1);
        } else {
            self.context_menus.clear();
        }
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position,
            content: ContextMenuContent::Items(items),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            open_left: false,
            hovered_item: None,
            native_request_id: None, open_up: false,
        });
        cx.notify();
    }
}
