use super::OxydeApp;
use crate::ui::components::input::TextInput;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct SearchCancellation(pub Arc<AtomicBool>);

#[derive(Clone)]
pub struct SearchInputSlide {
    expanded: bool,
    hover_suppressed: bool,
    from: f32,
    started: std::time::Instant,
    revision: u64,
}

impl Default for SearchInputSlide {
    fn default() -> Self {
        Self {
            expanded: false,
            hover_suppressed: false,
            from: 0.0,
            started: std::time::Instant::now(),
            revision: 0,
        }
    }
}

fn slide_progress(from: f32, expanded: bool, elapsed: f32) -> f32 {
    let t = elapsed.clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    from + (if expanded { 1.0 } else { 0.0 } - from) * eased
}

impl SearchInputSlide {
    fn set_expanded(&mut self, expanded: bool) {
        if self.expanded == expanded {
            return;
        }
        self.from = slide_progress(
            self.from,
            self.expanded,
            self.started.elapsed().as_secs_f32() / 0.25,
        );
        self.expanded = expanded;
        self.started = std::time::Instant::now();
        self.revision = self.revision.wrapping_add(1);
    }
}
impl Drop for SearchCancellation {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_search_path, search_path, slide_progress, SearchCancellation, SearchInputSlide,
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[test]
    fn input_slide_opens_closes_and_can_reverse_without_jumping() {
        assert_eq!(slide_progress(0.0, true, 0.0), 0.0);
        assert_eq!(slide_progress(0.0, true, 0.5), 0.5);
        assert_eq!(slide_progress(0.0, true, 1.0), 1.0);
        assert_eq!(slide_progress(1.0, false, 0.5), 0.5);
        assert_eq!(slide_progress(1.0, false, 1.0), 0.0);
        let mut slide = SearchInputSlide::default();
        slide.set_expanded(true);
        slide.started = std::time::Instant::now() - std::time::Duration::from_millis(125);
        slide.set_expanded(false);
        assert!((slide.from - 0.5).abs() < 0.02);
        assert_eq!(slide_progress(slide.from, false, 0.0), slide.from);
        assert_eq!(slide_progress(slide.from, false, 1.0), 0.0);
    }

    #[test]
    fn history_paths_preserve_unc_and_query_punctuation() {
        for (root, query) in [
            (r"C:\Users\Été ?", "us?*.json"),
            (r"\\server\share", "a\"b&c#d"),
        ] {
            assert_eq!(
                parse_search_path(&search_path(root, query)),
                Some((root.to_owned(), query.to_owned()))
            );
        }
        assert!(parse_search_path("C:\\").is_none());
        assert!(parse_search_path("search://broken").is_none());
    }

    #[test]
    fn advanced_history_keeps_filters_without_reinterpreting_quick_queries() {
        let options = oxyde_lib::commands::search::AdvancedSearchOptions {
            query: r"^Été\d".into(),
            regex: true,
            recursive: true,
            min_size: Some(1536),
            content: "résumé".into(),
            ..Default::default()
        };
        let root = r"\\server\share";
        let path = format!(
            "search://{}",
            serde_json::to_string(&("advanced", root, options)).unwrap()
        );
        assert_eq!(
            parse_search_path(&path),
            Some((root.into(), r"^Été\d".into()))
        );
        let (_, restored) = crate::ui::app::advanced_search::parse_advanced_path(&path).unwrap();
        assert_eq!(restored.min_size, Some(1536));
        assert!(restored.regex);
        assert!(restored.recursive);
        assert_eq!(restored.content, "résumé");
        assert!(
            crate::ui::app::advanced_search::parse_advanced_path(&search_path(
                root,
                "advanced-options:{}"
            ))
            .is_none()
        );
    }

    #[test]
    fn closing_last_search_state_cancels_worker() {
        let token = Arc::new(AtomicBool::new(false));
        let owner = Arc::new(SearchCancellation(token.clone()));
        let moved_tab = owner.clone();
        drop(owner);
        assert!(!token.load(Ordering::Relaxed));
        drop(moved_tab);
        assert!(token.load(Ordering::Relaxed));
    }

    #[test]
    fn revealing_result_expands_its_date_group_and_consumes_pending_path() {
        let mut tab = crate::ui::workspace::TabState::new("tab".into(), "folder".into());
        tab.group_by_date = true;
        let modified = chrono::Local::now().timestamp_millis() as u64;
        let group = crate::ui::date_groups::DateGroup::for_timestamp(
            modified,
            chrono::Local::now().date_naive(),
        );
        tab.collapsed_date_groups.insert(group);
        tab.files.push(oxyde_lib::models::FileEntry {
            name: "result.json".into(),
            path: "folder/result.json".into(),
            modified,
            ..Default::default()
        });
        tab.pending_reveal = Some("folder/result.json".into());
        tab.update_cache();
        tab.reveal_pending_item(800.0);
        assert!(tab.pending_reveal.is_none());
        assert!(!tab.collapsed_date_groups.contains(&group));
    }
}

#[derive(Clone)]
pub struct QuickSearchState {
    pub root: String,
    pub query: String,
    pub running: bool,
    pub limited: bool,
    pub cancellation: Arc<SearchCancellation>,
}

pub fn search_path(root: &str, query: &str) -> String {
    format!(
        "search://{}",
        serde_json::to_string(&(root, query)).expect("search strings")
    )
}

pub fn parse_search_path(path: &str) -> Option<(String, String)> {
    serde_json::from_str(path.strip_prefix("search://")?)
        .ok()
        .or_else(|| {
            super::advanced_search::parse_advanced_path(path)
                .map(|(root, options)| (root, options.query))
        })
}

pub fn search_label(path: &str, i18n: &crate::ui::i18n::I18n) -> Option<String> {
    let (root, query) = parse_search_path(path)?;
    let mut args = FluentArgs::new();
    args.set("query", query);
    args.set(
        "folder",
        std::path::Path::new(&root)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or(root),
    );
    Some(
        i18n.text_with_args(Message::QuickSearchTitle, &args)
            .to_string(),
    )
}

impl OxydeApp {
    pub(crate) fn render_quick_search_control(
        &self,
        pane_id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = self.state.find_pane(pane_id).expect("pane").active_tab();
        let theme = *cx.theme();
        let handle = cx.entity();
        let id = tab.id.clone();
        let pane_id = pane_id.to_owned();
        let expanded = tab.search_slide.expanded && tab.search_input.is_some();
        let slide_from = tab.search_slide.from;
        let slide_revision = tab.search_slide.revision;
        let outside_handle = handle.clone();
        let outside_id = id.clone();
        let hover_handle = handle.clone();
        let hover_id = id.clone();
        let hover_pane = pane_id.clone();
        div()
            .id(SharedString::from(format!("quick-search-control-{id}")))
            .flex()
            .flex_none()
            .items_center()
            .child(
                div()
                    .flex_none()
                    .w(rems(0.0625))
                    .h(rems(1.25))
                    .mr(rems(0.25))
                    .bg(theme.border),
            )
            .when_some(tab.search_input.clone(), |bar, input| {
                let field = div()
                    .flex_none()
                    .overflow_hidden()
                    .h(rems(1.5))
                    .child(div().w(rems(10.0)).h_full().pl(rems(0.25)).child(input));
                bar.child(field.with_animation(
                    SharedString::from(format!("search-slide-{id}-{slide_revision}")),
                    Animation::new(std::time::Duration::from_millis(250)),
                    move |field, delta| {
                        let fraction = slide_progress(slide_from, expanded, delta);
                        field.w(rems(10.0 * fraction)).opacity(fraction)
                    },
                ))
            })
            .child(
                div()
                    .id(SharedString::from(format!("quick-search-{id}")))
                    .size(rems(1.5))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(move |style| style.bg(theme.control_hover))
                    .tooltip(crate::ui::components::tooltip::Tooltip::with_meta(
                        cx.i18n().text(Message::QuickSearch).to_string(),
                        cx.i18n().text(Message::QuickSearchCurrentFolder),
                    ))
                    .child(
                        svg()
                            .path("icons/search.svg")
                            .size(rems(1.0))
                            .text_color(theme.text_muted),
                    )
                    .on_hover(move |hovered, window, cx| {
                        hover_handle.update(cx, |this, cx| {
                            let Some(tab) = this.state.find_tab_mut(&hover_id) else {
                                return;
                            };
                            if !*hovered {
                                tab.search_slide.hover_suppressed = false;
                            } else if !tab.search_slide.expanded
                                && !tab.search_slide.hover_suppressed
                                && this.context_menus.is_empty()
                            {
                                this.open_quick_search(&hover_pane, window, cx);
                            }
                        });
                    })
                    .on_click(move |_, window, cx| {
                        handle.update(cx, |this, cx| {
                            if expanded {
                                if let Some(tab) = this.state.find_tab_mut(&id) {
                                    tab.search_slide.hover_suppressed = true;
                                }
                                this.collapse_quick_search(&id, cx);
                                window.focus(&this.focus_handle, cx);
                            } else {
                                this.open_quick_search(&pane_id, window, cx);
                            }
                        });
                    }),
            )
            .when(expanded, |control| {
                control.on_mouse_down_out(move |_, window, cx| {
                    outside_handle.update(cx, |this, cx| {
                        if !this.context_menus.is_empty() {
                            return;
                        }
                        let focused = this
                            .state
                            .find_tab_mut(&outside_id)
                            .and_then(|tab| tab.search_input.as_ref())
                            .is_some_and(|input| input.read(cx).focus_handle.is_focused(window));
                        this.collapse_quick_search(&outside_id, cx);
                        if focused {
                            window.focus(&this.focus_handle, cx);
                        }
                    });
                })
            })
            .into_any_element()
    }
    pub(crate) fn render_quick_search_banner(
        &self,
        pane_id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = self.state.find_pane(pane_id).expect("pane").active_tab();
        let search = tab.quick_search.as_ref().expect("search");
        let theme = *cx.theme();
        let handle = cx.entity();
        let id = tab.id.clone();
        let stop_handle = handle.clone();
        let stop_id = id.clone();
        div()
            .flex()
            .items_center()
            .gap(rems(0.75))
            .px(rems(1.0))
            .h(rems(2.0))
            .flex_none()
            .bg(theme.search_banner)
            .border_b(rems(0.0625))
            .border_color(theme.border)
            .text_size(rems(0.8125))
            .child(
                svg()
                    .path("icons/search.svg")
                    .size(rems(1.0))
                    .text_color(theme.accent),
            )
            .child(
                div()
                    .flex_none()
                    .rounded_sm()
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .px(rems(0.375))
                    .py(rems(0.125))
                    .text_size(rems(0.625))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child(cx.i18n().text(Message::QuickSearchResults).to_uppercase()),
            )
            .child(
                div()
                    .id(SharedString::from(format!("search-query-{id}")))
                    .min_w_0()
                    .truncate()
                    .text_color(theme.accent)
                    .tooltip(crate::ui::components::tooltip::Tooltip::text(
                        search.root.clone(),
                    ))
                    .child(format!("'{}'", search.query)),
            )
            .child(div().flex_1())
            .when(search.running || search.limited, |bar| {
                bar.child(
                    div()
                        .id(SharedString::from(format!("search-status-{stop_id}")))
                        .text_color(theme.text_muted)
                        .when(search.running, |status| {
                            status
                                .tooltip(crate::ui::components::tooltip::Tooltip::text(
                                    cx.i18n().text(Message::QuickSearchStop).to_string(),
                                ))
                                .on_click(move |_, _, cx| {
                                    stop_handle.update(cx, |this, cx| {
                                        this.stop_quick_search(&stop_id, cx)
                                    });
                                })
                        })
                        .child(cx.i18n().text(if search.running {
                            Message::QuickSearchRunning
                        } else {
                            Message::QuickSearchLimited
                        })),
                )
            })
            .child(
                div()
                    .id(SharedString::from(format!("clear-search-{id}")))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(rems(0.375))
                    .h(rems(1.5))
                    .m(rems(0.25))
                    .px(rems(0.5))
                    .rounded_sm()
                    .bg(theme.control_hover)
                    .text_color(theme.text)
                    .hover(move |style| style.bg(theme.control_hover))
                    .child(
                        svg()
                            .path("icons/x.svg")
                            .size(rems(0.875))
                            .text_color(theme.text),
                    )
                    .child(cx.i18n().text(Message::QuickSearchClear))
                    .on_click(move |_, window, cx| {
                        handle.update(cx, |this, cx| {
                            this.clear_quick_search(&id, cx);
                            window.focus(&this.focus_handle, cx);
                        });
                    }),
            )
            .into_any_element()
    }

    fn stop_quick_search(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(tab) = self.state.find_tab_mut(id) {
            tab.search_revision = tab.search_revision.wrapping_add(1);
            if let Some(search) = &mut tab.quick_search {
                search.cancellation.0.store(true, Ordering::Relaxed);
                search.running = false;
            }
            tab.load_status = crate::ui::workspace::TabLoadStatus::Ready;
        }
        cx.notify();
    }
    fn search_tab_location(&self, tab_id: &str) -> Option<(String, usize)> {
        let mut found = None;
        self.state.layout.visit_panes(&mut |pane| {
            if let Some(index) = pane.tabs.iter().position(|tab| tab.id == tab_id) {
                found = Some((pane.id.clone(), index));
            }
        });
        found
    }
    pub(crate) fn open_quick_search(
        &mut self,
        pane_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.state.find_pane(pane_id) else {
            return;
        };
        let tab_id = pane.active_tab().id.clone();
        if let Some(input) = pane.active_tab().search_input.clone() {
            let focus = input.read(cx).focus_handle.clone();
            if let Some(tab) = self.state.find_tab_mut(&tab_id) {
                tab.search_slide.set_expanded(true);
            }
            self.activate_pane(pane_id, cx);
            window.focus(&focus, cx);
            cx.notify();
            return;
        }
        let initial = pane.active_tab().search_text.clone();
        let handle = cx.entity();
        let change = handle.clone();
        let enter = handle.clone();
        let escape = handle.clone();
        let change_id = tab_id.clone();
        let enter_id = tab_id.clone();
        let escape_id = tab_id.clone();
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                cx.i18n().text(Message::QuickSearch),
                initial,
                move |text, _, cx| {
                    change.update(cx, |this, cx| {
                        this.change_search_text(&change_id, text.to_string(), cx)
                    });
                },
                move |_, cx| {
                    enter.update(cx, |this, cx| this.submit_quick_search(&enter_id, cx));
                },
                move |window, cx| {
                    escape.update(cx, |this, cx| {
                        this.clear_quick_search(&escape_id, cx);
                        window.focus(&this.focus_handle, cx);
                    });
                },
                move |input, selected, position, window, cx| {
                    handle.update(cx, |this, cx| {
                        this.open_text_context_menu(input, selected, position, window, cx)
                    });
                },
                true,
                true,
                false,
                None,
                window,
            )
        });
        let focus = input.read(cx).focus_handle.clone();
        if let Some(tab) = self.state.find_tab_mut(&tab_id) {
            tab.search_input = Some(input);
            tab.search_slide = Default::default();
            tab.search_slide.set_expanded(true);
        }
        self.activate_pane(pane_id, cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn change_search_text(&mut self, tab_id: &str, text: String, cx: &mut Context<Self>) {
        let Some(tab) = self.state.find_tab_mut(tab_id) else {
            return;
        };
        tab.search_text = text;
        tab.search_revision = tab.search_revision.wrapping_add(1);
        if let Some(search) = &tab.quick_search {
            search.cancellation.0.store(true, Ordering::Relaxed);
        }
        let revision = tab.search_revision;
        let id = tab_id.to_owned();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(350))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .state
                    .find_tab_mut(&id)
                    .is_some_and(|tab| tab.search_revision == revision)
                {
                    this.submit_quick_search(&id, cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn collapse_quick_search(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        if let Some(tab) = self.state.find_tab_mut(tab_id) {
            tab.search_slide.set_expanded(false);
        }
        cx.notify();
    }

    fn submit_quick_search(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        let Some(tab) = self.state.find_tab_mut(tab_id) else {
            return;
        };
        let query = tab.search_text.trim().to_owned();
        if query.is_empty() {
            self.clear_quick_search(tab_id, cx);
            return;
        }
        let root = parse_search_path(&tab.path)
            .map(|(root, _)| root)
            .unwrap_or_else(|| tab.path.clone());
        let path = search_path(&root, &query);
        if tab.path == path
            && tab
                .quick_search
                .as_ref()
                .is_some_and(|search| !search.cancellation.0.load(Ordering::Relaxed))
        {
            return;
        }
        if !tab.path.starts_with("search://") {
            tab.back_stack.push(tab.path.clone());
        }
        tab.forward_stack.clear();
        tab.path = path.clone();
        tab.filter_name.clear();
        tab.filter_types.clear();
        tab.filter_sizes.clear();
        tab.filter_dates.clear();
        tab.list_scroll = UniformListScrollHandle::new();
        tab.list_scroll_h.set_offset(point(px(0.0), px(0.0)));
        let index = self.search_tab_location(tab_id);
        if let Some((pane, index)) = index {
            self.request_quick_search(&pane, index, path, cx);
        }
        self.save_workspace(cx);
    }

    pub(crate) fn clear_quick_search(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        let Some(tab) = self.state.find_tab_mut(tab_id) else {
            return;
        };
        let root = parse_search_path(&tab.path).map(|(root, _)| root);
        tab.search_revision = tab.search_revision.wrapping_add(1);
        tab.search_text.clear();
        tab.search_input = None;
        if let Some(search) = tab.quick_search.take() {
            search.cancellation.0.store(true, Ordering::Relaxed);
        }
        let location = self.search_tab_location(tab_id);
        if let (Some(root), Some((pane, index))) = (root, location) {
            if self
                .state
                .find_pane(&pane)
                .is_some_and(|pane| pane.active_tab_index == index)
            {
                self.navigate_to(&pane, root, cx);
            } else {
                // A delayed clear still belongs to the tab where the text was edited.
                if let Some(tab) = self.state.find_tab_mut(tab_id) {
                    tab.back_stack.push(tab.path.clone());
                    tab.forward_stack.clear();
                    tab.path = root.clone();
                    tab.files.clear();
                    tab.cached_filtered_files.clear();
                    tab.selected_paths.clear();
                    tab.selection_anchor = None;
                    tab.load_status = crate::ui::workspace::TabLoadStatus::Loading;
                }
                self.request_tab_files(&pane, index, root, true, cx);
                self.save_workspace(cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn request_quick_search(
        &mut self,
        pane_id: &str,
        index: usize,
        path: String,
        cx: &mut Context<Self>,
    ) {
        let Some((root, query)) = parse_search_path(&path) else {
            return;
        };
        let Some(tab) = self
            .state
            .find_pane_mut(pane_id)
            .and_then(|pane| pane.tabs.get_mut(index))
        else {
            return;
        };
        if let Some(search) = tab.quick_search.take() {
            search.cancellation.0.store(true, Ordering::Relaxed);
        }
        let options =
            super::advanced_search::parse_advanced_path(&path).map(|(_, options)| options);
        let query = options.as_ref().map(|o| o.query.clone()).unwrap_or(query);
        let cancellation = Arc::new(AtomicBool::new(false));
        tab.quick_search = Some(QuickSearchState {
            root: root.clone(),
            query: query.clone(),
            running: true,
            limited: false,
            cancellation: Arc::new(SearchCancellation(cancellation.clone())),
        });
        tab.search_text = query.clone();
        tab.files.clear();
        tab.update_cache();
        tab.selected_paths.clear();
        tab.selection_anchor = None;
        tab.load_status = crate::ui::workspace::TabLoadStatus::Loading;
        let id = tab.id.clone();
        self.pending_file_requests.remove(&id);
        let receiver = if let Some(options) = options {
            oxyde_lib::commands::search::advanced_search(
                root,
                options,
                self.config.search_limit as usize,
                self.show_hidden,
                self.show_system,
                cancellation.clone(),
            )
        } else {
            oxyde_lib::commands::search::quick_search(
                root,
                query,
                self.config.search_limit as usize,
                self.show_hidden,
                self.show_system,
                cancellation.clone(),
            )
        };
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            if cancellation.load(Ordering::Relaxed) {
                break;
            }
            let mut events = Vec::new();
            loop {
                match receiver.try_recv() {
                    Ok(event) => events.push(event),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        if !events.iter().any(|event| {
                            !matches!(
                                event,
                                oxyde_lib::commands::search::QuickSearchEvent::Batch(_)
                            )
                        }) {
                            events.push(oxyde_lib::commands::search::QuickSearchEvent::Failed(
                                "Search worker disconnected".into(),
                            ));
                        }
                        break;
                    }
                }
            }
            if events.is_empty() {
                continue;
            }
            let done = events.iter().any(|event| {
                !matches!(
                    event,
                    oxyde_lib::commands::search::QuickSearchEvent::Batch(_)
                )
            });
            if this
                .update(cx, |this, cx| {
                    let Some(tab) = this.state.find_tab_mut(&id) else {
                        return;
                    };
                    if tab.path != path
                        || tab.quick_search.as_ref().is_none_or(|search| {
                            !Arc::ptr_eq(&search.cancellation.0, &cancellation)
                        })
                    {
                        return;
                    }
                    for event in events {
                        use oxyde_lib::commands::search::QuickSearchEvent;
                        match event {
                            QuickSearchEvent::Batch(files) => {
                                tab.files.extend(files);
                            }
                            QuickSearchEvent::Finished { limited } => {
                                if let Some(search) = &mut tab.quick_search {
                                    search.running = false;
                                    search.limited = limited;
                                }
                                tab.load_status = crate::ui::workspace::TabLoadStatus::Ready;
                            }
                            QuickSearchEvent::Failed(error) => {
                                log::warn!("Quick search failed: {error}");
                                if let Some(search) = &mut tab.quick_search {
                                    search.running = false;
                                }
                                tab.load_status = crate::ui::workspace::TabLoadStatus::Error(
                                    Message::QuickSearchFailed,
                                );
                            }
                        }
                    }
                    tab.sort_files();
                    tab.update_cache();
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
            if done {
                break;
            }
        })
        .detach();
        cx.notify();
    }
}
