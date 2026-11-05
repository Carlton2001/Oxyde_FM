//! « Open With », as Nautilus offers it on Linux: the recommended applications in a
//! submenu, and a prompt listing every application, which may become the default.
use super::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::components::input::{TextInput, TextInputOptions};
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;
use oxyde_lib::models::{ApplicationChoice, ApplicationFit};

pub(crate) struct OpenWithPrompt {
    pub paths: Vec<String>,
    /// Missing while the applications load.
    pub choices: Option<Vec<ApplicationChoice>>,
    pub selected: Option<usize>,
    /// Filters the applications by name.
    pub input: Entity<TextInput>,
    /// Kept as typed: an input calling back cannot be read while it updates.
    pub query: String,
    /// « Always use for this file type ».
    pub remember: bool,
    pub focus: FocusHandle,
    pub scroll: ScrollHandle,
}

impl OpenWithPrompt {
    /// Indices of the choices matching the search, in display order.
    pub(crate) fn visible(&self) -> Vec<usize> {
        let query = oxyde_lib::utils::remove_accents(&self.query.trim().to_lowercase());
        self.choices.iter().flatten().enumerate()
            .filter(|(_, choice)| query.is_empty()
                || oxyde_lib::utils::remove_accents(&choice.name.to_lowercase()).contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    /// Keeps the selection on a visible choice, the first one otherwise.
    fn keep_selection_visible(&mut self) {
        let visible = self.visible();
        if !self.selected.is_some_and(|index| visible.contains(&index)) {
            self.selected = visible.first().copied();
        }
    }
}

/// Whether Oxyde offers « Open With » for `path`: a local file outside an archive.
pub(crate) fn offers(path: &str, is_dir: bool) -> bool {
    !is_dir
        && oxyde_lib::platform::platform().offers_open_with()
        && !path.contains("://")
        && oxyde_lib::utils::archive::split_virtual_path(path).is_none()
}

async fn load(paths: Vec<String>) -> Result<Vec<ApplicationChoice>, String> {
    tokio::task::spawn_blocking(move || oxyde_lib::platform::platform().open_with_choices(&paths))
        .await
        .unwrap_or_else(|error| Err(error.to_string()))
}

impl OxydeApp {
    /// The files of the selection « Open With » applies to, folders left out.
    pub(crate) fn open_with_paths(&self, clicked: &str) -> Vec<String> {
        let paths = self.context_action_paths(clicked);
        let Some(pane) = self.state.find_pane(&self.state.active_pane_id) else { return paths };
        let files: Vec<_> = pane.active_tab().files.iter()
            .filter(|file| paths.contains(&file.path) && offers(&file.path, file.is_dir))
            .map(|file| file.path.clone())
            .collect();
        if files.is_empty() { paths } else { files }
    }

    /// The submenu: the recommended applications, then the prompt for any other.
    /// Opens once the applications are known: replacing a shown submenu's rows would
    /// leave a stale pointer highlight on the surface that grew under it.
    pub(crate) fn show_open_with_menu(&mut self, paths: Vec<String>, position: Point<Pixels>, open_left: bool, cx: &mut Context<Self>) {
        self.context_menus.truncate(1);
        cx.notify();
        let other = cx.i18n().text(Message::OpenWithOther).to_string();
        let label = cx.i18n().text(Message::OpenWith).to_string();
        let default = cx.i18n().text(Message::OpenWithDefault).to_string();
        cx.spawn(async move |this, cx| {
            let found = load(paths.clone()).await;
            let _ = this.update(cx, |this, cx| {
                // Still on « Open With », with no other submenu opened meanwhile.
                let still_hovered = this.context_menus.len() == 1 && this.context_menus[0].hovered_item
                    .zip(match &this.context_menus[0].content { ContextMenuContent::Items(items, _) => Some(items), _ => None })
                    .and_then(|(index, items)| items.get(index))
                    .is_some_and(|item| item.label == label);
                if !still_hovered {
                    return;
                }
                let handle = cx.entity();
                let mut items: Vec<DropdownItem> = found.unwrap_or_else(|error| {
                    log::warn!("Cannot list applications: {error}");
                    Vec::new()
                })
                .into_iter()
                .filter(|choice| choice.fit == ApplicationFit::Recommended)
                .map(|choice| {
                    let (handle, paths, id) = (handle.clone(), paths.clone(), choice.id.clone());
                    let is_default = choice.is_default;
                    let item = DropdownItem::new(choice.name, move |_, app| {
                        handle.update(app, |this, cx| {
                            this.context_menus.clear();
                            this.open_with_application(id.clone(), paths.clone(), false, cx);
                        });
                    })
                    .icon(IconName::Image(choice.icon));
                    DropdownItem { badge: is_default.then(|| default.clone()), ..item }
                })
                .collect();
                if !items.is_empty() {
                    items.push(DropdownItem::separator());
                }
                items.push(DropdownItem::new(other.clone(), move |_, app| {
                    handle.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.show_open_with_prompt(paths.clone(), cx);
                    });
                }).icon(IconName::Image(None)));
                this.context_menus.push(ContextMenuInstance {
                    native_bounds: None,
                    position,
                    content: ContextMenuContent::Items(items, None),
                    scroll_handle: UniformListScrollHandle::new(),
                    width: px(0.0),
                    open_left,
                    hovered_item: None,
                    native_request_id: None,
                    open_up: false,
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_with_application(&mut self, id: String, paths: Vec<String>, remember: bool, cx: &mut Context<Self>) {
        for path in &paths {
            self.pause_viewer_media_for_external_open(path, cx);
        }
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || oxyde_lib::platform::platform().open_with(&id, &paths, remember))
                .await
                .unwrap_or_else(|error| Err(error.to_string()));
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.file_operation_notice = Some((Message::OperationFailed, error));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn show_open_with_prompt(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        if paths.is_empty() || self.open_with.is_some() {
            return;
        }
        // The field needs the main window, not the menu's.
        let Some(window) = self.main_window else { return };
        cx.spawn(async move |this, cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| this.open_open_with_prompt(paths, window, cx));
            });
        })
        .detach();
    }

    fn open_open_with_prompt(&mut self, paths: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_with.is_some() {
            return;
        }
        let handle = cx.entity();
        let (change, submit, cancel, menu) = (handle.clone(), handle.clone(), handle.clone(), handle);
        let placeholder = cx.i18n().text(Message::OpenWithSearch).to_string();
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                placeholder,
                String::new(),
                move |text, _, cx| change.update(cx, |this, cx| {
                    if let Some(prompt) = &mut this.open_with {
                        prompt.query = text.to_string();
                        prompt.keep_selection_visible();
                        prompt.scroll.set_offset(Point::default());
                        cx.notify();
                    }
                }),
                move |_, cx| submit.update(cx, |this, cx| this.answer_open_with(true, cx)),
                move |_, cx| cancel.update(cx, |this, cx| this.answer_open_with(false, cx)),
                move |input, selected, point, window, cx| {
                    menu.update(cx, |this, cx| this.open_text_context_menu(input, selected, point, window, cx))
                },
                TextInputOptions { clearable: true, ..Default::default() },
                window,
            )
        });
        self.open_with = Some(OpenWithPrompt {
            paths: paths.clone(),
            choices: None,
            selected: None,
            input,
            query: String::new(),
            remember: false,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let found = load(paths.clone()).await;
            let _ = this.update(cx, |this, cx| {
                let Some(prompt) = this.open_with.as_mut().filter(|prompt| prompt.paths == paths) else { return };
                prompt.choices = Some(found.unwrap_or_else(|error| {
                    log::warn!("Cannot list applications: {error}");
                    Vec::new()
                }));
                prompt.keep_selection_visible();
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens with the selected application when `open`, then closes the prompt.
    pub(crate) fn answer_open_with(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.open_with.take() else { return };
        let chosen = prompt.selected.filter(|index| prompt.visible().contains(index)).and_then(|index| prompt.choices.as_ref()?.get(index));
        if let (true, Some(choice)) = (open, chosen) {
            self.open_with_application(choice.id.clone(), prompt.paths.clone(), prompt.remember, cx);
        }
        cx.notify();
    }

    pub(crate) fn select_open_with(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(prompt) = &mut self.open_with {
            prompt.selected = Some(index);
            cx.notify();
        }
    }

    /// Moves the selection by `step` visible rows, kept in view.
    pub(crate) fn move_open_with_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(prompt) = &mut self.open_with else { return };
        let visible = prompt.visible();
        if visible.is_empty() {
            return;
        }
        let next = match prompt.selected.and_then(|index| visible.iter().position(|shown| *shown == index)) {
            Some(position) => position.saturating_add_signed(step).min(visible.len() - 1),
            None => 0,
        };
        prompt.selected = Some(visible[next]);
        prompt.scroll.scroll_to_item(crate::ui::components::open_with_dialog::row_child(prompt, visible[next]));
        cx.notify();
    }

    pub(crate) fn toggle_open_with_remember(&mut self, cx: &mut Context<Self>) {
        if let Some(prompt) = &mut self.open_with {
            prompt.remember = !prompt.remember;
            cx.notify();
        }
    }
}
