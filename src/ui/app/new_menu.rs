use super::context_menus::{NATIVE_MENU_TIMEOUT, NativeMenuLoad};
use super::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::components::icons::{IconType, get_icon_type};
use crate::ui::i18n::{I18n, I18nContext, Message};
use gpui::*;
use oxyde_lib::platform::{ShellNewCommand, ShellNewTemplate, platform};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CATALOG_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(super) struct Catalog {
    request: Option<(Source, Instant)>,
    rows: Vec<Row>,
    load: Option<Task<()>>,
    pending_menu: Option<(UniformListScrollHandle, Target)>,
}

#[derive(Clone, PartialEq, Eq)]
struct Source {
    folder: String,
    icons: bool,
}

#[derive(Clone)]
enum Action {
    Shell(ShellNewCommand),
    Template(Arc<ShellNewTemplate>),
}

struct Row {
    label: String,
    icon: IconName,
    action: Action,
}

fn file_icon(extension: Option<&str>, system: bool) -> IconName {
    let Some(extension) = extension else {
        return IconName::FileType {
            path: "oxyde_file_generic".into(),
            icon: IconType::File,
            system,
        };
    };
    let extension = extension.trim_start_matches('.');
    let icon = get_icon_type(&format!("file.{extension}"), false, None, false, false);
    IconName::FileType {
        path: format!("oxyde_ext_{extension}"),
        icon,
        system,
    }
}

fn load_rows(source: &Source) -> Vec<Row> {
    let entries = platform().shell_new_menu(&source.folder, source.icons);
    match entries {
        Ok(entries) if !entries.is_empty() => entries
            .into_iter()
            .map(|entry| {
                let label = entry.command.label.clone();
                let icon = if let Some(bitmap) = entry.icon {
                    IconName::ShellIcon(Arc::new(RenderImage::new(vec![image::Frame::new(bitmap)])))
                } else {
                    file_icon(entry.extension.as_deref(), source.icons)
                };
                Row {
                    label,
                    icon,
                    action: Action::Shell(entry.command),
                }
            })
            .collect(),
        other => {
            if let Err(error) = other {
                log::warn!("Cannot read the Windows New submenu: {error}");
            }
            platform()
                .shell_new_templates()
                .unwrap_or_default()
                .into_iter()
                .map(|template| Row {
                    label: template.label.clone(),
                    icon: file_icon(Some(&template.extension), source.icons),
                    action: Action::Template(Arc::new(template)),
                })
                .collect()
        }
    }
}

#[derive(Clone)]
struct Target {
    pane_id: String,
    tab_id: String,
    path: String,
}

impl Target {
    fn still_writable(&self, app: &OxydeApp) -> bool {
        app.state.find_pane(&self.pane_id).is_some_and(|pane| {
            let tab = pane.active_tab();
            tab.id == self.tab_id
                && tab.path == self.path
                && tab.archive_root.is_none()
                && !app.is_read_only_location(&self.pane_id, &self.path)
        })
    }
}

impl OxydeApp {
    pub(crate) fn windows_new_menu_item(
        &self,
        handle: Entity<Self>,
        pane_id: &str,
        i18n: &I18n,
    ) -> Option<DropdownItem> {
        let tab = self.state.find_pane(pane_id)?.active_tab();
        if tab.archive_root.is_some() || !Path::new(&tab.path).is_absolute() {
            return None;
        }
        let target = Target {
            pane_id: pane_id.to_owned(),
            tab_id: tab.id.clone(),
            path: tab.path.clone(),
        };
        Some(
            DropdownItem::new(i18n.text(Message::NewItem).to_string(), |_, _| {})
                .icon(IconName::Plus)
                .stay_open(true)
                .has_submenu(true)
                .on_hover(move |position, open_left, app| {
                    handle.update(app, |this, cx| {
                        this.open_new_menu(target.clone(), position, open_left, cx)
                    });
                }),
        )
    }

    pub(super) fn refresh_new_menu_catalog(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane(&self.state.active_pane_id) {
            let tab = pane.active_tab();
            if tab.archive_root.is_none() && Path::new(&tab.path).is_absolute() {
                self.request_new_menu_catalog(tab.path.clone(), cx);
            }
        }
    }

    fn request_new_menu_catalog(&mut self, folder: String, cx: &mut Context<Self>) {
        let source = Source {
            folder,
            icons: self.use_system_icons(),
        };
        let catalog = &mut self.new_menu_catalog;
        let same_source = catalog
            .request
            .as_ref()
            .is_some_and(|(key, _)| key == &source);
        if same_source
            && (catalog.load.is_some()
                || catalog
                    .request
                    .as_ref()
                    .is_some_and(|(_, time)| time.elapsed() < CATALOG_REFRESH_INTERVAL))
        {
            return;
        }
        if !same_source {
            catalog.load = None;
            catalog.rows.clear();
            catalog.pending_menu = None;
            catalog.request = None;
        }
        let Some(load) = NativeMenuLoad::start() else {
            return;
        };
        catalog.request = Some((source.clone(), Instant::now()));
        let timer = cx.background_executor().timer(NATIVE_MENU_TIMEOUT);
        catalog.load = Some(cx.spawn(async move |this, cx| {
            let worker = tokio::task::spawn_blocking(move || {
                let _load = load;
                load_rows(&source)
            });
            let result = futures::future::select(worker, timer).await;
            let _ = this.update(cx, |this, cx| {
                let catalog = &mut this.new_menu_catalog;
                catalog.load = None;
                match result {
                    futures::future::Either::Left((Ok(rows), _)) => catalog.rows = rows,
                    futures::future::Either::Left((Err(error), _)) => {
                        log::warn!("Windows New menu task failed: {error}")
                    }
                    futures::future::Either::Right(_) => {
                        log::warn!("Windows New menu load timed out")
                    }
                }
                if let Some((scroll, target)) = catalog.pending_menu.take() {
                    if target.still_writable(this)
                        && this
                            .context_menus
                            .get(1)
                            .is_some_and(|menu| Rc::ptr_eq(&menu.scroll_handle.0, &scroll.0))
                    {
                        this.context_menus[1].content =
                            ContextMenuContent::Items(this.new_menu_items(&target, cx));
                        cx.notify();
                    }
                }
            });
        }));
    }

    fn open_new_menu(
        &mut self,
        target: Target,
        position: Point<Pixels>,
        open_left: bool,
        cx: &mut Context<Self>,
    ) {
        if self.context_menus.is_empty() || !target.still_writable(self) {
            return;
        }
        self.request_new_menu_catalog(target.path.clone(), cx);
        let scroll_handle = UniformListScrollHandle::new();
        let catalog = &mut self.new_menu_catalog;
        if catalog.load.is_some() && catalog.rows.is_empty() {
            catalog.pending_menu = Some((scroll_handle.clone(), target.clone()));
        }
        let items = self.new_menu_items(&target, cx);
        self.context_menus.truncate(1);
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position,
            content: ContextMenuContent::Items(items),
            scroll_handle,
            width: px(0.0),
            open_left,
            open_up: false,
            hovered_item: None,
            native_request_id: None,
        });
        cx.notify();
    }

    fn new_menu_items(&self, target: &Target, cx: &Context<Self>) -> Vec<DropdownItem> {
        let app = cx.entity();
        let i18n = cx.i18n();
        let system = self.use_system_icons();
        let mut items = Vec::with_capacity(3 + self.new_menu_catalog.rows.len());
        for (label, folder, icon) in [
            (
                Message::NewFolder,
                true,
                IconName::FileType {
                    path: String::new(),
                    icon: IconType::Folder,
                    system,
                },
            ),
            (
                Message::NewEmptyFile,
                false,
                if system {
                    IconName::FileType {
                        path: "oxyde_file_generic".into(),
                        icon: IconType::File,
                        system,
                    }
                } else {
                    IconName::FilePlus
                },
            ),
        ] {
            let (app, target) = (app.clone(), target.clone());
            items.push(
                DropdownItem::new(i18n.text(label).to_string(), move |_, cx| {
                    app.update(cx, |this, cx| {
                        if target.still_writable(this) {
                            if folder {
                                this.create_new_folder(&target.pane_id, cx);
                            } else {
                                this.create_new_file(&target.pane_id, cx);
                            }
                        }
                    });
                })
                .icon(icon),
            );
        }
        if !self.new_menu_catalog.rows.is_empty() {
            items.push(DropdownItem::separator());
        }
        for row in &self.new_menu_catalog.rows {
            let (app, target, action) = (app.clone(), target.clone(), row.action.clone());
            items.push(
                DropdownItem::new(row.label.clone(), move |_, cx| {
                    app.update(cx, |this, cx| {
                        if target.still_writable(this) {
                            match action.clone() {
                                Action::Shell(command) => this.run_native_menu_action(
                                    vec![target.path.clone()],
                                    Some(target.tab_id.clone()),
                                    true,
                                    move |path| {
                                        platform().execute_shell_new_command(&path[0], &command)
                                    },
                                    cx,
                                ),
                                Action::Template(template) => {
                                    this.create_new_template(&target.pane_id, template, cx)
                                }
                            }
                        }
                    });
                })
                .icon(row.icon.clone()),
            );
        }
        items
    }
}
