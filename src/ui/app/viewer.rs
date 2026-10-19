mod media;
mod render;
mod window;
pub(crate) use window::close_viewer_windows;

use super::OxydeApp;
use crate::ui::components::tooltip::PreviewTask;
use gpui::*;
use oxyde_lib::models::FileEntry;
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Default)]
struct Source {
    tab: String,
    selected: Arc<HashSet<String>>,
    files: Arc<Vec<FileEntry>>,
}

pub(crate) struct Viewer {
    owner: WeakEntity<OxydeApp>,
    tab_id: String,
    source: Source,
    files: Vec<FileEntry>,
    index: usize,
    preview: Option<Entity<PreviewTask>>,
    preparation: Option<Task<()>>,
    media: Option<media::MediaSession>,
    native_media: bool,
    audio: bool,
    detached: bool,
    detached_window: Option<WindowHandle<window::ViewerWindow>>,
    blocked: bool,
    focus: FocusHandle,
    focus_pending: bool,
    _subscriptions: Vec<Subscription>,
}

impl OxydeApp {
    pub(crate) fn stop_viewers(&mut self, cx: &mut Context<Self>) {
        for viewer in self.viewers.values() {
            viewer.update(cx, |viewer, _| {
                viewer.media = None;
                viewer.preparation = None;
                viewer.preview = None;
            });
        }
        self.viewers.clear();
    }

    pub(crate) fn open_viewer(&mut self, cx: &mut Context<Self>) {
        self.context_menus.clear();
        self.settings_open = false;
        let source = self
            .state
            .find_pane(&self.state.active_pane_id)
            .and_then(|pane| self.viewers.get(&pane.active_tab().id))
            .map(|viewer| viewer.read(cx).source.clone())
            .unwrap_or_else(|| Source::capture(self, &Source::default()));
        let id = self.state.add_viewer();
        self.sync_viewers(cx);
        if let Some(viewer) = id.and_then(|id| self.viewers.get(&id)) {
            viewer.update(cx, |viewer, cx| {
                viewer.set_source(source, cx);
                viewer.focus_pending = true;
            });
        }
        self.reconcile_watchers(cx);
        self.save_workspace(cx);
        cx.notify();
    }

    pub(super) fn sync_viewers(&mut self, cx: &mut Context<Self>) {
        let mut ids = HashSet::new();
        self.state.layout.visit_panes(&mut |pane| {
            ids.extend(
                pane.tabs
                    .iter()
                    .filter(|tab| tab.is_viewer())
                    .map(|tab| tab.id.clone()),
            );
        });
        self.viewers.retain(|id, viewer| {
            if ids.contains(id) {
                true
            } else {
                viewer.update(cx, |viewer, cx| {
                    viewer.media = None;
                    if let Some(handle) = viewer.detached_window.take() {
                        cx.defer(move |cx| {
                            let _ = handle.update(cx, |_, window, cx| {
                                crate::ui::components::tool_window::close_transients(window, cx);
                                window.remove_window();
                            });
                        });
                    }
                });
                false
            }
        });
        if ids.iter().all(|id| self.viewers.contains_key(id)) {
            return;
        }
        let source = Source::capture(self, &Source::default());
        for id in ids {
            if self.viewers.contains_key(&id) {
                continue;
            }
            let owner = cx.entity();
            let initial = source.clone();
            let viewer = cx.new(|cx| Viewer::new(owner, id.clone(), initial, cx));
            self.viewers.insert(id, viewer);
        }
    }
}

impl Source {
    fn same_revision(&self, other: &Self) -> bool {
        self.tab == other.tab
            && Arc::ptr_eq(&self.selected, &other.selected)
            && Arc::ptr_eq(&self.files, &other.files)
    }
    fn selected_files(&self) -> Vec<FileEntry> {
        if self.selected.is_empty() {
            return Vec::new();
        }
        self.files
            .iter()
            .filter(|file| self.selected.contains(&file.path))
            .cloned()
            .collect()
    }
    fn capture(owner: &OxydeApp, previous: &Source) -> Self {
        let from_tab = |tab: &crate::ui::workspace::TabState| Self {
            tab: tab.id.clone(),
            selected: if previous.tab == tab.id && *previous.selected == tab.selected_paths {
                previous.selected.clone()
            } else {
                Arc::new(tab.selected_paths.clone())
            },
            files: tab.cached_filtered_files.clone(),
        };
        let mut tab = owner
            .state
            .find_pane(&owner.state.active_pane_id)
            .map(|pane| pane.active_tab())
            .filter(|tab| !tab.is_viewer())
            .map(from_tab);
        if tab.is_none() {
            owner.state.layout.visit_panes(&mut |pane| {
                if let Some(candidate) = pane
                    .tabs
                    .iter()
                    .find(|tab| tab.id == previous.tab && !tab.is_viewer())
                {
                    tab = Some(from_tab(candidate));
                }
            });
        }
        if tab.is_none() {
            owner.state.layout.visit_panes(&mut |pane| {
                if tab.is_none() && !pane.active_tab().is_viewer() {
                    tab = Some(from_tab(pane.active_tab()));
                }
            });
        }
        tab.unwrap_or_default()
    }
}

impl Viewer {
    fn new(
        owner: Entity<OxydeApp>,
        tab_id: String,
        source: Source,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&owner, |this, owner, cx| {
            let app = owner.read(cx);
            let source = Source::capture(app, &this.source);
            this.blocked = !this.detached && (app.settings_open || app.dragging_tab.is_some());
            this.set_source(source, cx);
            this.update_visibility();
            cx.notify();
        });
        let mut this = Self {
            owner: owner.downgrade(),
            tab_id,
            source: Source::default(),
            files: Vec::new(),
            index: 0,
            preview: None,
            preparation: None,
            media: None,
            native_media: false,
            audio: false,
            detached: false,
            detached_window: None,
            blocked: false,
            focus: cx.focus_handle(),
            focus_pending: false,
            _subscriptions: vec![subscription],
        };
        this.set_source(source, cx);
        this
    }

    fn set_source(&mut self, source: Source, cx: &mut Context<Self>) {
        if self.source.same_revision(&source) {
            return;
        }
        self.preparation = None;
        if source.selected.is_empty() || source.files.len() <= 1024 {
            let files = source.selected_files();
            self.source = source;
            self.publish_files(files, cx);
            return;
        }
        // A large directory scan must not block selection or playback controls.
        let retained = self
            .files
            .get(self.index)
            .filter(|file| source.selected.contains(&file.path))
            .cloned();
        self.files = retained.into_iter().collect();
        self.index = 0;
        if self.files.is_empty() {
            self.load_current(cx);
        }
        self.source = source.clone();
        let expected = source.clone();
        let work = cx
            .background_executor()
            .spawn(async move { source.selected_files() });
        self.preparation = Some(cx.spawn(async move |this, cx| {
            let files = work.await;
            let _ = this.update(cx, |this, cx| {
                if !this.source.same_revision(&expected) {
                    return;
                }
                this.preparation = None;
                this.publish_files(files, cx);
                cx.notify();
            });
        }));
    }

    fn publish_files(&mut self, files: Vec<FileEntry>, cx: &mut Context<Self>) {
        let old = self.files.get(self.index).cloned();
        self.files = files;
        self.index = old
            .as_ref()
            .and_then(|old| self.files.iter().position(|file| file.path == old.path))
            .unwrap_or(0);
        let unchanged = old
            .as_ref()
            .zip(self.files.get(self.index))
            .is_some_and(|(old, file)| {
                old.path == file.path
                    && old.content_stamp == file.content_stamp
                    && old.modified == file.modified
                    && old.size == file.size
            });
        if !unchanged {
            self.load_current(cx);
        }
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.files.len() < 2 {
            return;
        }
        let next = if forward {
            (self.index + 1).min(self.files.len() - 1)
        } else {
            self.index.saturating_sub(1)
        };
        if next == self.index {
            return;
        }
        self.index = next;
        self.load_current(cx);
        cx.notify();
    }

    fn load_current(&mut self, cx: &mut Context<Self>) {
        self.preview = None;
        self.media = None;
        self.native_media = false;
        self.audio = false;
        let Some(file) = self.files.get(self.index) else {
            return;
        };
        if file.is_dir || file.archive_path.is_some() {
            return;
        }
        use crate::ui::components::icons::{
            get_preview_type, PreviewType, AUDIO_EXTENSIONS, VIDEO_EXTENSIONS,
        };
        let extension = file
            .name
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        self.audio = AUDIO_EXTENSIONS.contains(extension.as_str())
            || file.native_kind == Some(oxyde_lib::models::file_entry::NativeFileKind::Audio);
        self.native_media = self.audio
            || VIDEO_EXTENSIONS.contains(extension.as_str())
            || matches!(
                file.native_kind,
                Some(
                    oxyde_lib::models::file_entry::NativeFileKind::Audio
                        | oxyde_lib::models::file_entry::NativeFileKind::Video
                )
            );
        if self.native_media {
            if !cfg!(target_os = "windows") {
                self.native_media = false;
            }
            return;
        }
        let kind = get_preview_type(&file.name);
        if kind != PreviewType::None {
            let path = file.path.clone();
            self.preview = Some(cx.new(|cx| PreviewTask::new_viewer(path, kind, cx)));
        }
    }

    fn activate(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let id = &self.tab_id;
        let _ = self.owner.update(cx, |app, cx| {
            let mut pane_id = None;
            app.state.layout.visit_panes(&mut |pane| {
                if pane.tabs.iter().any(|tab| &tab.id == id) {
                    pane_id = Some(pane.id.clone());
                }
            });
            if let Some(pane) = pane_id {
                app.activate_pane(&pane, cx);
            }
        });
    }
}

#[cfg(test)]
#[path = "viewer/tests/selection.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "viewer/tests/native_viewer.rs"]
mod native_tests;
