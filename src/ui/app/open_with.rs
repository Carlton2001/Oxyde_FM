//! « Open With », as Nautilus offers it on Linux: the recommended applications in a
//! submenu, and a prompt listing every application, which may become the default.
use super::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;
use oxyde_lib::models::{ApplicationChoice, ApplicationFit};

pub(crate) struct OpenWithPrompt {
    pub paths: Vec<String>,
    /// Missing while the applications load.
    pub choices: Option<Vec<ApplicationChoice>>,
    pub selected: Option<usize>,
    /// « Always use for this file type ».
    pub remember: bool,
    pub focus: FocusHandle,
    pub scroll: ScrollHandle,
}

/// Whether Oxyde offers « Open With » for `path`: a local file outside an archive.
pub(crate) fn offers(path: &str, is_dir: bool) -> bool {
    !is_dir
        && oxyde_lib::platform::platform().offers_open_with()
        && !path.contains("://")
        && oxyde_lib::utils::archive::split_virtual_path(path).is_none()
}

fn load(paths: Vec<String>) -> impl std::future::Future<Output = Result<Vec<ApplicationChoice>, String>> {
    async move {
        tokio::task::spawn_blocking(move || oxyde_lib::platform::platform().open_with_choices(&paths))
            .await
            .unwrap_or_else(|error| Err(error.to_string()))
    }
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
    pub(crate) fn show_open_with_menu(&mut self, paths: Vec<String>, position: Point<Pixels>, open_left: bool, cx: &mut Context<Self>) {
        let loading = DropdownItem { is_dimmed: true, ..DropdownItem::new(cx.i18n().text(Message::OpenWithLoading).to_string(), |_, _| {}) };
        let scroll = UniformListScrollHandle::new();
        self.context_menus.truncate(1);
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position,
            content: ContextMenuContent::Items(vec![loading], None),
            scroll_handle: scroll.clone(),
            width: px(0.0),
            open_left,
            hovered_item: None,
            native_request_id: None,
            open_up: false,
        });
        cx.notify();
        let other = cx.i18n().text(Message::OpenWithOther).to_string();
        cx.spawn(async move |this, cx| {
            let found = load(paths.clone()).await;
            let _ = this.update(cx, |this, cx| {
                let Some(menu) = this.context_menus.get_mut(1) else { return };
                if !std::rc::Rc::ptr_eq(&menu.scroll_handle.0, &scroll.0) {
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
                    DropdownItem::new(choice.name, move |_, app| {
                        handle.update(app, |this, cx| {
                            this.context_menus.clear();
                            this.open_with_application(id.clone(), paths.clone(), false, cx);
                        });
                    })
                    .icon(IconName::Image(choice.icon))
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
                menu.content = ContextMenuContent::Items(items, None);
                menu.width = px(0.0);
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
        self.open_with = Some(OpenWithPrompt {
            paths: paths.clone(),
            choices: None,
            selected: None,
            remember: false,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let found = load(paths.clone()).await;
            let _ = this.update(cx, |this, cx| {
                let Some(prompt) = this.open_with.as_mut().filter(|prompt| prompt.paths == paths) else { return };
                let choices = found.unwrap_or_else(|error| {
                    log::warn!("Cannot list applications: {error}");
                    Vec::new()
                });
                prompt.selected = (!choices.is_empty()).then_some(0);
                prompt.choices = Some(choices);
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens with the selected application when `open`, then closes the prompt.
    pub(crate) fn answer_open_with(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.open_with.take() else { return };
        let chosen = prompt.selected.and_then(|index| prompt.choices.as_ref()?.get(index));
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

    /// Moves the selection by `step` rows, kept in view.
    pub(crate) fn move_open_with_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(prompt) = &mut self.open_with else { return };
        let count = prompt.choices.as_ref().map_or(0, Vec::len);
        if count == 0 {
            return;
        }
        let next = prompt.selected.map_or(0, |index| index.saturating_add_signed(step).min(count - 1));
        prompt.selected = Some(next);
        prompt.scroll.scroll_to_item(crate::ui::components::open_with_dialog::row_child(prompt, next));
        cx.notify();
    }

    pub(crate) fn toggle_open_with_remember(&mut self, cx: &mut Context<Self>) {
        if let Some(prompt) = &mut self.open_with {
            prompt.remember = !prompt.remember;
            cx.notify();
        }
    }
}
