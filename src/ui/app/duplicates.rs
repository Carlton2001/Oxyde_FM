use super::OxydeApp;
use crate::ui::{
    components::input::TextInput,
    i18n::{FluentArgs, I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use oxyde_lib::{
    commands::duplicates::{
        cancel_find_duplicates, find_duplicates, DuplicateGroup, DuplicateSearchOptions,
        DuplicateSearchState,
    },
    models::DriveInfo,
};
use std::{collections::HashSet, sync::Arc};


pub struct DuplicateWindow {
    owner: WeakEntity<OxydeApp>,
    locations: Vec<(String, String, String)>,
    location_icons: Vec<&'static str>,
    selected: HashSet<String>,
    options: [bool; 3],
    groups: Arc<Vec<DuplicateGroup>>,
    collapsed: HashSet<usize>,
    filter: Entity<TextInput>,
    query: String,
    filtered: Vec<usize>,
    results_scroll: ScrollHandle,
    sources_scroll: ScrollHandle,
    running: bool,
    searched: bool,
    error: Option<String>,
    cancellation: Arc<DuplicateSearchState>,
    limit: usize,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
    resize_task: Option<Task<()>>,
    filter_task: Option<Task<()>>,
    filtering: bool,
    hovered_result: Option<String>,
    result_tooltip: Option<(String, AnyView)>,
    tooltip_task: Option<Task<()>>,
}

impl Drop for DuplicateWindow {
    fn drop(&mut self) {
        cancel_find_duplicates(&self.cancellation);
    }
}

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
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(160.0), px(140.0)),
                        size(
                            px(dimensions[0].clamp(660.0, 1800.0)),
                            px(dimensions[1].clamp(420.0, 1100.0)),
                        ),
                    ))),
                    window_min_size: Some(size(px(660.0), px(420.0))),
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
                    crate::ui::format::drive_label(&d.label, &d.path, cx.i18n()),
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
                        this.limit = 100;
                        cx.notify();
                    });
                },
                |_, _| {},
                |_, _| {},
                |_, _, _, _, _| {},
                false,
                true,
                false,
                None,
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
            results_scroll: ScrollHandle::new(),
            sources_scroll: ScrollHandle::new(),
            running: false,
            searched: false,
            error: None,
            cancellation: Arc::new(DuplicateSearchState::new()),
            limit: 100,
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

    fn search(&mut self, cx: &mut Context<Self>) {
        if self.running || self.selected.is_empty() || !self.options.iter().any(|v| *v) {
            return;
        }
        self.running = true;
        self.searched = true;
        self.error = None;
        self.limit = 100;
        self.results_scroll.set_offset(point(px(0.0), px(0.0)));
        self.cancellation = Arc::new(DuplicateSearchState::new());
        let progress_token = self.cancellation.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let keep_running = this
                .update(cx, |this, cx| {
                    if !this.running || !Arc::ptr_eq(&this.cancellation, &progress_token) {
                        return false;
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !keep_running {
                break;
            }
        })
        .detach();
        let cancellation = self.cancellation.clone();
        let paths = self.selected.iter().cloned().collect();
        let options = DuplicateSearchOptions {
            by_name: self.options[0],
            by_size: self.options[1],
            by_content: self.options[2],
        };
        let runtime = tokio::runtime::Handle::current();
        let worker = cx
            .background_executor()
            .spawn(async move { runtime.block_on(find_duplicates(&cancellation, paths, options)) });
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                this.running = false;
                if !this
                    .cancellation
                    .0
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    match result {
                        Ok(groups) => {
                            this.groups = Arc::new(groups);
                            this.filtered.clear();
                            this.update_filter(cx);
                            this.collapsed.clear();
                        }
                        Err(e) => this.error = Some(e.to_string()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn button(id: impl Into<ElementId>, label: SharedString, cx: &App) -> Stateful<Div> {
        Self::styled_button(id, label, false, cx)
    }

    fn styled_button(id: impl Into<ElementId>, label: SharedString, primary: bool, cx: &App) -> Stateful<Div> {
        crate::ui::components::tool_window::action_button(id, label, primary, cx)
    }

    fn update_filter(&mut self, cx: &mut Context<Self>) {
        let groups = self.groups.clone();
        let query = self.query.clone();
        self.filtering = true;
        self.filter_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let snapshot = groups.clone();
            let needle = query.trim().to_string();
            let indices = cx
                .background_executor()
                .spawn(async move {
                    snapshot
                        .iter()
                        .enumerate()
                        .filter(|(_, group)| {
                            needle.is_empty()
                                || group.files.iter().any(|file| {
                                    file.path.to_lowercase().contains(&needle)
                                        || file.name.to_lowercase().contains(&needle)
                                })
                        })
                        .map(|(index, _)| index)
                        .collect()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if Arc::ptr_eq(&this.groups, &groups) && this.query == query {
                    this.filtered = indices;
                    this.filtering = false;
                    cx.notify();
                }
            });
        }));
    }
}

impl Render for DuplicateWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        window.set_window_title(cx.i18n().text(Message::ToolDuplicates).as_ref());
        let system_icons = self
            .owner
            .upgrade()
            .is_some_and(|owner| owner.read(cx).use_system_icons());
        if let Some(owner) = self.owner.upgrade() {
            window.set_rem_size(px(owner.read(cx).ui_rem_size() as f32));
        }
        let placeholder = cx.i18n().text(Message::DuplicateFilter);
        self.filter
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        let section_title = |icon: &'static str, message: Message| {
            div()
                .flex()
                .items_center()
                .gap(rems(0.5))
                .mb(rems(0.5))
                .text_size(rems(0.75))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(
                    svg()
                        .path(icon)
                        .size(rems(0.875))
                        .text_color(theme.text_muted),
                )
                .child(cx.i18n().text(message).to_uppercase())
        };
        let mut sidebar = div()
            .id("duplicate-sidebar")
            .w(rems(17.5))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.sources_scroll)
            .px(rems(1.25))
            .py(rems(0.8))
            .flex()
            .flex_col()
            .border_r(rems(0.0625))
            .border_color(theme.border)
            .child(section_title(
                "icons/database.svg",
                Message::DuplicateLocations,
            ));
        let mut locations = div()
            .flex_none()
            .rounded(rems(0.375))
            .border(rems(0.0625))
            .border_color(theme.border)
            .overflow_hidden();
        for (index, (path, label, disk)) in self.locations.iter().enumerate() {
            let selected = self.selected.contains(path);
            let current = label.is_empty();
            let target = path.clone();
            let label = if current {
                cx.i18n().text(Message::DuplicateCurrent)
            } else {
                label.clone().into()
            };
            let (media, physical) = disk.split_once("  ").unwrap_or((disk.as_str(), ""));
            let media_name = if media.contains("SSD") {
                "SSD"
            } else if media.contains("HDD") {
                "HDD"
            } else {
                media
            };
            let badge_bg = if media.contains("SSD") {
                theme.media_ssd
            } else if media.contains("HDD") {
                theme.media_hdd
            } else if self.location_icons[index] == "icons/usb.svg" {
                theme.media_removable
            } else if self.location_icons[index] == "icons/network.svg" {
                theme.media_network
            } else {
                theme.badge_surface
            };
            let mut badges = div().flex().items_center().gap(rems(0.375));
            if !media_name.is_empty() {
                badges = badges.child(
                    div()
                        .rounded(rems(0.2))
                        .px(rems(0.35))
                        .py(rems(0.0625))
                        .text_size(rems(0.625))
                        .font_weight(FontWeight::MEDIUM)
                        .bg(badge_bg)
                        .text_color(theme.text_on_badge)
                        .child(media_name.to_string()),
                );
            }
            if !physical.is_empty() {
                badges = badges.child(
                    div()
                        .rounded(rems(0.2))
                        .px(rems(0.35))
                        .py(rems(0.0625))
                        .text_size(rems(0.625))
                        .bg(theme.badge_surface)
                        .text_color(theme.text_muted)
                        .child(physical.to_uppercase()),
                );
            }
            let mut content = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(rems(0.2))
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if selected { theme.accent } else { theme.text })
                        .child(label),
                )
                .child(badges);
            if current {
                content = content.child(
                    div()
                        .id(("duplicate-source-path", index))
                        .truncate()
                        .text_size(rems(0.75))
                        .text_color(theme.text_muted)
                        .tooltip(crate::ui::components::tooltip::Tooltip::text(path.clone()))
                        .child(path.clone()),
                );
            }
            locations = locations.child(
                div()
                    .id(("location", index))
                    .flex_none()
                    .tab_index(0)
                    .focus_visible(move |s| s.bg(theme.selected))
                    .px(rems(0.75))
                    .py(rems(0.625))
                    .flex()
                    .items_center()
                    .gap(rems(0.75))
                    .bg(if selected {
                        theme.selected
                    } else {
                        theme.background
                    })
                    .border_b(if index + 1 < self.locations.len() {
                        rems(0.0625)
                    } else {
                        rems(0.0)
                    })
                    .border_color(theme.border)
                    .hover(move |s| {
                        s.bg(if selected {
                            theme.selected
                        } else {
                            theme.hover
                        })
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.running {
                            if !this.selected.remove(&target) {
                                this.selected.insert(target.clone());
                            }
                            cx.notify();
                        }
                    }))
                    .child(
                        svg()
                            .path(self.location_icons[index])
                            .size(rems(0.875))
                            .flex_none()
                            .text_color(theme.text_muted),
                    )
                    .child(content),
            );
        }
        sidebar = sidebar.child(locations);
        if self.selected.len() > 1 {
            sidebar = sidebar.child(
                div()
                    .flex_none()
                    .p(rems(0.75))
                    .mt(rems(0.125))
                    .rounded(rems(0.5))
                    .border(rems(0.0625))
                    .border_color(theme.warning_border)
                    .bg(theme.warning_background)
                    .text_color(theme.warning_text)
                    .child(cx.i18n().text(Message::DuplicateWarning)),
            );
        }
        sidebar = sidebar.child(div().mt(rems(1.75)).child(section_title(
            "icons/settings-2.svg",
            Message::DuplicateSettings,
        )));
        let mut options = div()
            .flex_none()
            .rounded(rems(0.5))
            .border(rems(0.0625))
            .border_color(theme.border)
            .bg(theme.header)
            .p(rems(0.75))
            .flex()
            .flex_col()
            .gap(rems(0.6));
        for (index, message) in [
            Message::DuplicateName,
            Message::DuplicateSize,
            Message::DuplicateContent,
        ]
        .into_iter()
        .enumerate()
        {
            let checked = self.options[index];
            options = options.child(
                div()
                    .id(("criterion", index))
                    .tab_index(0)
                    .flex()
                    .items_center()
                    .gap(rems(0.5))
                    .focus_visible(move |s| s.bg(theme.selected))
                    .child(
                        div()
                            .size(rems(1.0))
                            .rounded(rems(0.25))
                            .border(rems(0.0625))
                            .border_color(if checked { theme.accent } else { theme.border })
                            .bg(if checked {
                                theme.accent
                            } else {
                                theme.background
                            })
                            .when(checked, |s| {
                                s.child(
                                    svg()
                                        .path("icons/check.svg")
                                        .size(rems(0.875))
                                        .text_color(theme.text_on_accent),
                                )
                            }),
                    )
                    .child(cx.i18n().text(message))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.running {
                            this.options[index] = !this.options[index];
                            let options = this.options;
                            let _ = this.owner.update(cx, |owner, cx| {
                                owner.config.duplicate_search_options = Some(serde_json::json!(options));
                                owner.save_preferences(cx);
                            });
                            cx.notify();
                        }
                    })),
            );
        }
        sidebar = sidebar.child(options);
        let mut args = FluentArgs::new();
        args.set("groups", self.groups.len() as i64);
        args.set(
            "group_count",
            crate::ui::format::format_number(self.groups.len() as u64, cx.i18n().language()),
        );
        let file_count = self.groups.iter().map(|g| g.files.len()).sum::<usize>();
        args.set("file_count", file_count as i64);
        args.set(
            "file_number",
            crate::ui::format::format_number(file_count as u64, cx.i18n().language()),
        );
        args.set(
            "files",
            self.groups.iter().map(|g| g.files.len()).sum::<usize>() as i64,
        );
        let mut results = div()
            .id("duplicate-results")
            .track_scroll(&self.results_scroll)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(rems(1.0))
            .flex()
            .flex_col()
            .gap(rems(0.5));
        if self.running {
            let progress = self.cancellation.1.lock().unwrap().clone();
            let mut a = FluentArgs::new();
            a.set(
                "current",
                crate::ui::format::format_number(progress.current as u64, cx.i18n().language()),
            );
            a.set(
                "total",
                crate::ui::format::format_number(progress.total as u64, cx.i18n().language()),
            );
            results = results.child(cx.i18n().text_with_args(
                if progress.stage == "Scanning" {
                    Message::DuplicateScanning
                } else {
                    Message::DuplicateHashing
                },
                &a,
            ));
        }
        if let Some(error) = &self.error {
            let mut a = FluentArgs::new();
            a.set("error", error.as_str());
            results = results.child(
                div()
                    .text_color(theme.error)
                    .child(cx.i18n().text_with_args(Message::DuplicateFailed, &a)),
            );
        }
        if self.filtered.is_empty() && !self.running && !self.filtering && self.error.is_none() {
            results = results.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(rems(1.0))
                    .text_color(theme.text_muted)
                    .child(
                        svg()
                            .path("icons/search.svg")
                            .size(rems(3.0))
                            .text_color(theme.icon_disabled),
                    )
                    .child(div().max_w(rems(18.75)).text_center().child(cx.i18n().text(
                        if self.searched {
                            Message::DuplicateEmpty
                        } else {
                            Message::DuplicateIdle
                        },
                    ))),
            );
        }
        for &index in self.filtered.iter().take(self.limit) {
            let group = &self.groups[index];
            let mut a = FluentArgs::new();
            a.set("index", (index + 1) as i64);
            a.set("count", group.files.len() as i64);
            let mut card = div()
                .flex_none()
                .rounded(rems(0.625))
                .border(rems(0.0625))
                .border_color(theme.border)
                .bg(theme.surface)
                .overflow_hidden()
                .child(
                    div()
                        .id(("group", index))
                        .tab_index(0)
                        .focus_visible(move |s| s.border(rems(0.0625)).border_color(theme.accent))
                        .p(rems(0.5))
                        .bg(theme.header)
                        .items_center()
                        .border_b(rems(0.0625))
                        .border_color(theme.border)
                        .hover(move |s| s.bg(theme.hover))
                        .flex()
                        .gap(rems(0.75))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.collapsed.remove(&index) {
                                this.collapsed.insert(index);
                            }
                            cx.notify();
                        }))
                        .child(
                            svg()
                                .path(if self.collapsed.contains(&index) {
                                    "icons/chevron-right.svg"
                                } else {
                                    "icons/chevron-down.svg"
                                })
                                .size(rems(1.0))
                                .text_color(theme.text_muted),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(cx.i18n().text_with_args(Message::DuplicateGroup, &a)),
                        )
                        .child(
                            div()
                                .rounded(rems(0.75))
                                .px(rems(0.5))
                                .py(rems(0.125))
                                .min_w(rems(1.5))
                                .bg(theme.accent)
                                .text_color(theme.text_on_accent)
                                .text_size(rems(0.75))
                                .font_weight(FontWeight::MEDIUM)
                                .child(crate::ui::format::format_number(
                                    group.files.len() as u64,
                                    cx.i18n().language(),
                                )),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .px(rems(0.6))
                                .py(rems(0.2))
                                .rounded(rems(0.375))
                                .border(rems(0.0625))
                                .border_color(theme.border)
                                .bg(theme.background)
                                .text_size(rems(0.75))
                                .child(crate::ui::format::format_size(
                                    group.size,
                                    false,
                                    true,
                                    cx.i18n().language(),
                                )),
                        ),
                );
            if !self.collapsed.contains(&index) {
                let mut body = div().p(rems(0.5)).flex().flex_col().gap(rems(0.25));
                for (file_index, file) in group.files.iter().enumerate() {
                    let target = file.path.clone();
                    let parent = std::path::Path::new(&target)
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let destination = parent.clone();
                    let hover_path = target.clone();
                    let hover_name = file.name.clone();
                    let hover_parent = parent.clone();
                    let tooltip = self
                        .result_tooltip
                        .as_ref()
                        .filter(|(path, _)| path == &target)
                        .map(|(_, view)| view.clone());
                    body = body.child(
                        div()
                            .id(ElementId::Name(
                                format!("duplicate-file-{index}-{file_index}").into(),
                            ))
                            .relative()
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if !*hovered && this.hovered_result.as_ref() != Some(&hover_path) { return; }
                                this.tooltip_task = None;
                                this.result_tooltip = None;
                                if *hovered {
                                    this.hovered_result = Some(hover_path.clone());
                                    let path = hover_path.clone(); let name = hover_name.clone(); let parent = hover_parent.clone();
                                    this.tooltip_task = Some(cx.spawn(async move |this, cx| {
                                        cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                                        let _ = this.update(cx, |this, cx| {
                                            if this.hovered_result.as_ref() == Some(&path) {
                                                let view = crate::ui::components::tooltip::Tooltip::duplicate_result(name, parent, cx);
                                                this.result_tooltip = Some((path, view)); cx.notify();
                                            }
                                        });
                                    }));
                                } else if this.hovered_result.as_ref() == Some(&hover_path) {
                                    this.hovered_result = None;
                                }
                                cx.notify();
                            }))
                            .child(canvas(move |bounds, window, _| {
                                if let Some(view) = tooltip {
                                    window.set_tooltip(AnyTooltip {
                                        view,
                                        mouse_position: point(bounds.left(), bounds.bottom() + crate::ui::scale::rem_pixels(rems(0.25))),
                                        check_visible_and_update: std::rc::Rc::new(move |_, window, _| bounds.contains(&window.mouse_position())),
                                    });
                                }
                            }, |_, _, _, _| {}).absolute().size_full())
                            .tab_index(0)
                            .focus_visible(move |s| {
                                s.border(rems(0.0625)).border_color(theme.accent)
                            })
                            .px(rems(1.0))
                            .py(rems(0.75))
                            .rounded(rems(0.625))
                            .flex()
                            .items_center()
                            .gap(rems(0.75))
                            .hover(move |s| s.bg(theme.hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tooltip_task = None;
                                this.hovered_result = None;
                                this.result_tooltip = None;
                                cx.notify();
                                let _ = this.owner.update(cx, |owner, cx| {
                                    let pane = owner.state.active_pane_id.clone();
                                    owner.navigate_to(&pane, destination.clone(), cx);
                                    if let Some(pane) = owner.state.find_pane_mut(&pane) {
                                        let tab = pane.active_tab_mut();
                                        tab.selected_paths.clear();
                                        tab.selected_paths.insert(target.clone());
                                        tab.selection_anchor = Some(target.clone());
                                        tab.pending_reveal = Some(target.clone());
                                    }
                                    cx.notify();
                                });
                            }))
                            .child(crate::ui::components::icons::render_file_icon(
                                file.path.clone(),
                                crate::ui::components::icons::entry_icon(file),
                                rems(1.75),
                                theme.text_muted,
                                system_icons,
                                true,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(file.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "duplicate-parent-{index}-{file_index}"
                                            )))
                                            .text_color(theme.text_muted)
                                            .text_size(rems(0.75))
                                            .truncate()
                                            .child(parent),
                                    ),
                            ),
                    );
                }
                card = card.child(body);
            }
            results = results.child(card);
        }
        if self.filtered.len() > self.limit {
            results = results.child(
                Self::button("duplicate-more", cx.i18n().text(Message::DuplicateMore), cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.limit += 100;
                        cx.notify();
                    })),
            );
        }
        let disabled = self.selected.is_empty() || !self.options.iter().any(|v| *v);
        div()
            .id("duplicate-window")
            .track_focus(&self.focus)
            .tab_group()
            .tab_stop(false)
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "tab" => {
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                        cx.stop_propagation();
                    }
                    "escape" => {
                        if this.running {
                            cancel_find_duplicates(&this.cancellation);
                            cx.notify();
                        } else {
                            window.remove_window();
                        }
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.focus.is_focused(window) && event.keystroke.key == "enter" {
                    this.search(cx);
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25))
            .child(crate::ui::components::tool_window::header(
                "icons/copy.svg", cx.i18n().text(Message::ToolDuplicates), window.is_maximized(), true, cx,
                { let cancellation = self.cancellation.clone(); move |window, _| {
                    cancel_find_duplicates(&cancellation); window.remove_window();
                } },
            ))
            .child(
                div().flex_1().min_h_0().flex().child(sidebar).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .p(rems(1.0))
                                .border_b(rems(0.0625))
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .gap(rems(1.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .h(rems(2.5))
                                        .border(rems(0.0625))
                                        .border_color(theme.border)
                                        .rounded(rems(0.375))
                                        .flex()
                                        .items_center()
                                        .pl(rems(0.75))
                                        .child(
                                            svg()
                                                .path("icons/search.svg")
                                                .size(rems(0.875))
                                                .flex_none()
                                                .text_color(theme.text_muted),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .h_full()
                                                .child(self.filter.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .flex()
                                        .flex_col()
                                        .items_end()
                                        .gap(rems(0.1))
                                        .child(
                                            div().font_weight(FontWeight::MEDIUM).child(
                                                cx.i18n().text_with_args(
                                                    Message::DuplicateGroups,
                                                    &args,
                                                ),
                                            ),
                                        )
                                        .child(
                                            div().text_color(theme.text_muted).child(
                                                cx.i18n()
                                                    .text_with_args(Message::DuplicateFiles, &args),
                                            ),
                                        ),
                                ),
                        )
                        .child(results),
                ),
            )
            .child(
                div()
                    .border_t(rems(0.0625))
                    .border_color(theme.border)
                    .p(rems(0.625))
                    .flex()
                    .justify_end()
                    .gap(rems(0.625))
                    .child(
                        Self::button("duplicate-cancel", cx.i18n().text(Message::Cancel), cx)
                            .on_click(cx.listener(|this, _, w, cx| {
                                if this.running {
                                    cancel_find_duplicates(&this.cancellation);
                                    cx.notify();
                                } else {
                                    w.remove_window();
                                }
                            })),
                    )
                    .child(
                        Self::styled_button(
                            "duplicate-search",
                            "".into(),
                            !disabled && !self.running,
                            cx,
                        )
                        .bg(if disabled || self.running {
                            theme.header
                        } else {
                            theme.accent
                        })
                        .text_color(if disabled || self.running {
                            theme.text_muted
                        } else {
                            theme.text_on_accent
                        })
                        .child(svg().path("icons/search.svg").size(rems(0.875)).text_color(
                            if disabled || self.running {
                                theme.text_muted
                            } else {
                                theme.text_on_accent
                            },
                        ))
                        .child(cx.i18n().text(Message::AdvancedSearch))
                        .on_click(cx.listener(|this, _, _, cx| this.search(cx))),
                    ),
            )
    }
}
