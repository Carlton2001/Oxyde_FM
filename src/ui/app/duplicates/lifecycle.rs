use super::DuplicateWindow;
use super::super::OxydeApp;
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use gpui::*;
use oxyde_lib::commands::duplicates::{cancel_find_duplicates, DuplicateSearchState};
use oxyde_lib::models::DriveInfo;
use std::{collections::HashSet, sync::Arc};

/// Secondary searches must not keep Oxyde alive after its main window closes.
pub(crate) fn close_duplicate_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<DuplicateWindow>() {
            let _ = handle.update(cx, |this, window, _| {
                cancel_find_duplicates(&this.cancellation);
                window.remove_window();
            });
        }
    }
}

impl OxydeApp {
    pub(crate) fn open_duplicates(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.context_menus.clear();
        for handle in cx.windows() {
            if let Some(handle) = handle.downcast::<DuplicateWindow>() {
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                });
                cx.notify();
                return;
            }
        }
        let owner = cx.entity().downgrade();
        let drives = self.state.drives.clone();
        let root = self
            .state
            .find_pane(&self.state.active_pane_id)
            .map(|p| p.active_tab().path.clone())
            .unwrap_or_default();
        let options = self
            .config
            .duplicate_search_options
            .as_ref()
            .and_then(|v| serde_json::from_value::<[bool; 3]>(v.clone()).ok())
            .unwrap_or([false, true, true]);
        let dimensions = self
            .config
            .duplicate_search_window_size
            .as_ref()
            .and_then(|v| serde_json::from_value::<[f32; 2]>(v.clone()).ok())
            .filter(|v| v.iter().all(|x| x.is_finite() && *x > 0.0))
            .unwrap_or([960.0, 700.0]);
        let title = cx.i18n().text(Message::ToolDuplicates);
        // GPUI draws a newly opened window before open_window returns. Its render
        // reads the owner, so creation must wait until this OxydeApp update ends.
        cx.defer(move |cx| {
            if owner.upgrade().is_none() {
                return;
            }
            let result = cx.open_window(
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    window_bounds: Some(crate::ui::app::tool_geometry::restore("icons/copy.svg", Bounds::new(
                        point(px(160.0), px(140.0)),
                        size(
                            px(dimensions[0].clamp(660.0, 1800.0).max(crate::ui::scale::logical(660.0))),
                            px(dimensions[1].clamp(420.0, 1100.0).max(crate::ui::scale::logical(420.0))),
                        ),
                    ), cx)),
                    window_min_size: Some(size(px(crate::ui::scale::logical(660.0)), px(crate::ui::scale::logical(420.0)))),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some(title),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| DuplicateWindow::new(owner, drives, root, options, window, cx))
                },
            );
            if let Err(error) = result {
                log::error!("Cannot open duplicate search window: {error}");
            }
        });
        cx.notify();
    }
}

impl DuplicateWindow {
    fn new(
        owner: WeakEntity<OxydeApp>,
        drives: Vec<DriveInfo>,
        root: String,
        options: [bool; 3],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut locations: Vec<_> = drives
            .iter()
            .map(|d| {
                (
                    d.path.clone(),
                    crate::ui::format::drive_name(d, cx.i18n()),
                    Self::disk_label(d, cx),
                )
            })
            .collect();
        let mut location_icons: Vec<_> = drives
            .iter()
            .map(crate::ui::components::icons::drive_icon_path)
            .collect();
        let valid_root = !root.is_empty()
            && !root.contains("://")
            && !root.starts_with("::{")
            && root != "__network_vincinity__";
        let mut selected = HashSet::new();
        if valid_root {
            selected.insert(root.clone());
            let disk = drives
                .iter()
                .find(|d| root.to_lowercase().starts_with(&d.path.to_lowercase()))
                .map(|d| Self::disk_label(d, cx))
                .unwrap_or_default();
            locations.push((root, String::new(), disk));
            location_icons.push("icons/download.svg");
        } else if let Some(d) = drives.first() {
            selected.insert(d.path.clone());
        }
        let weak = cx.entity().downgrade();
        let filter = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                String::new(),
                move |value, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.query = value.to_lowercase();
                        this.update_filter(cx);
                        this.results_scroll.set_offset(point(px(0.0), px(0.0)));
                        cx.notify();
                    });
                },
                |_, _| {},
                |_, _| {},
                |_, _, _, _, _| {},
                TextInputOptions {
                    appearance: InputAppearance::Embedded,
                    clearable: true,
                    ..Default::default()
                },
                window,
            )
        });
        crate::ui::components::tool_window::install_icon("icons/copy.svg", window);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let bounds_subscription = cx.observe_window_bounds(window, |this, window, cx| {
            let bounds = window.bounds();
            let dimensions = [bounds.size.width.as_f32(), bounds.size.height.as_f32()];
            this.resize_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(350))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let _ = this.owner.update(cx, |owner, cx| {
                        owner.config.duplicate_search_window_size = Some(serde_json::json!(dimensions));
                        owner.save_preferences(cx);
                    });
                });
            }));
        });
        let mut subscriptions = vec![bounds_subscription];
        if let Some(main) = owner.upgrade() {
            subscriptions.push(cx.observe_release_in(&main, window, |this, _, window, _| {
                cancel_find_duplicates(&this.cancellation);
                window.remove_window();
            }));
        }
        let result_list = UniformListScrollHandle::new();
        let results_scroll = result_list.0.borrow().base_handle.clone();
        Self {
            owner,
            locations,
            location_icons,
            selected,
            options,
            groups: Arc::new(vec![]),
            collapsed: HashSet::new(),
            filter,
            query: String::new(),
            filtered: Vec::new(),
            result_rows: Arc::default(),
            result_revision: 0,
            result_list,
            scrollbar_origins: Default::default(),
            grab_offset: 0.0,
            scroll_drag: None,
            results_scroll,
            sources_scroll: ScrollHandle::new(),
            running: false,
            searched: false,
            error: None,
            cancellation: Arc::new(DuplicateSearchState::new()),
            focus,
            _subscriptions: subscriptions,
            resize_task: None,
            filter_task: None,
            filtering: false,
            hovered_result: None,
            result_tooltip: None,
            tooltip_task: None,
        }
    }

    fn disk_label(d: &DriveInfo, cx: &App) -> String {
        if d.drive_type == "remote" {
            return cx.i18n().text(Message::SidebarNetwork).to_string();
        }
        let physical = d
            .physical_id
            .as_deref()
            .map(|id| {
                let mut args = FluentArgs::new();
                args.set(
                    "disk",
                    id.trim_start_matches("Disk")
                        .trim_start_matches("DISK")
                        .trim()
                        .trim_start_matches('#'),
                );
                cx.i18n()
                    .text_with_args(Message::DuplicateDisk, &args)
                    .to_string()
            })
            .unwrap_or_default();
        format!("{}  {physical}", d.media_type.as_deref().unwrap_or(""))
            .trim()
            .to_string()
    }
}
