mod content;
mod image_interaction;
mod audio;
mod content_render;
mod media;
mod media_bar;
mod media_overlay;
mod video_layout;
mod fullscreen_controls;
mod markdown;
mod display_options;
mod pdf;
mod render;
mod scrollbar;
mod text;
mod window;
pub(crate) use window::close_viewer_windows;

use super::OxydeApp;
use gpui::*;
use oxyde_lib::models::FileEntry;
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Default)]
struct Source {
    tab: String,
    selected: Arc<HashSet<String>>,
    /// Keyboard cursor, then anchor: the selected file the Viewer shows.
    focus: Option<String>,
    files: Arc<Vec<FileEntry>>,
}

/// The shown file and the nearest media of the same kind on each side.
#[derive(Default)]
struct Resolved {
    file: Option<FileEntry>,
    previous: Option<String>,
    next: Option<String>,
}

pub(crate) struct Viewer {
    owner: WeakEntity<OxydeApp>,
    /// The owning tab; the Viewer never activates its pane.
    #[cfg_attr(not(test), allow(dead_code))]
    tab_id: String,
    source: Source,
    file: Option<FileEntry>,
    /// Same-kind media around the file in the source pane, for skipping.
    neighbors: (Option<String>, Option<String>),
    content: content::Content,
    markdown_rendered: bool,
    syntax_highlighting: bool,
    media_autoplay: bool,
    /// Bumped per opened file; stale loads never publish into a newer file.
    generation: u64,
    /// Generation of the image the Viewer currently wants, read by queued jobs.
    wanted: Arc<std::sync::atomic::AtomicU64>,
    loading: Option<Task<()>>,
    scroll: scrollbar::ListScroll,
    preparation: Option<Task<()>>,
    media: Option<media::MediaSession>,
    /// Position and pause state carried to the next player surface.
    resume: Option<(u64, bool)>,
    /// External opens in the current selection, including pending resolution.
    external_open_pauses: HashSet<String>,
    /// Progress bar track while seeking with the pointer.
    seeking: bool,
    seek_track: Bounds<Pixels>,
    native_media: bool,
    audio: bool,
    detached: bool,
    fullscreen_pending: bool,
    fullscreen_temporary: bool,
    image_view: image_interaction::ImageView,
    fullscreen_controls: fullscreen_controls::Controls,
    overlay: media_overlay::Overlay,
    detached_window: Option<WindowHandle<window::ViewerWindow>>,
    blocked: bool,
    focus: FocusHandle,
    focus_pending: bool,
    _subscriptions: Vec<Subscription>,
}

/// The Viewer's image session holds a large worker budget; it is released
/// whenever no image is shown, so PDF sessions and players can start at once.
fn release_image_decoder(cx: &mut App) {
    cx.background_executor()
        .spawn(async { oxyde_lib::platform::platform().release_viewer_images() })
        .detach();
}

impl OxydeApp {
    pub(crate) fn stop_viewers(&mut self, cx: &mut Context<Self>) {
        for viewer in self.viewers.values() {
            viewer.update(cx, |viewer, cx| {
                viewer.close_fullscreen_controls(cx);
                viewer.close_overlay(cx);
                viewer.media = None;
                viewer.preparation = None;
                viewer.loading = None;
                viewer.replace_content(content::Content::None, cx);
            });
        }
        self.viewers.clear();
        release_image_decoder(cx);
    }

    pub(crate) fn viewer_open(&self) -> bool {
        self.state.viewer_tab_id().is_some()
    }

    /// The title-bar button opens the single Viewer or closes it.
    pub(crate) fn toggle_viewer(&mut self, cx: &mut Context<Self>) {
        let pane = self
            .state
            .viewer_tab_id()
            .and_then(|tab| self.state.viewer_pane_id(&tab));
        match pane {
            Some(pane) => {
                self.close_tab(&pane, 0, cx);
                self.sync_viewers(cx);
                cx.notify();
            }
            None => self.open_viewer(cx),
        }
    }

    pub(crate) fn open_viewer(&mut self, cx: &mut Context<Self>) {
        if self.viewer_open() {
            return;
        }
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
            viewer.update(cx, |viewer, cx| viewer.set_source(source, cx));
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
                    viewer.close_fullscreen_controls(cx);
                viewer.close_overlay(cx);
                    viewer.media = None;
                    viewer.loading = None;
                    // Closing frees the GPU textures of its image or PDF pages.
                    viewer.replace_content(content::Content::None, cx);
                    release_image_decoder(cx);
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
            let options = (self.config.viewer_markdown_rendered, self.config.viewer_syntax_highlighting, self.config.viewer_media_autoplay);
            let viewer = cx.new(|cx| Viewer::new(owner, id.clone(), initial, options, cx));
            self.viewers.insert(id, viewer);
        }
    }
}

impl Source {
    fn same_revision(&self, other: &Self) -> bool {
        self.tab == other.tab
            && Arc::ptr_eq(&self.selected, &other.selected)
            && self.focus == other.focus
            && Arc::ptr_eq(&self.files, &other.files)
    }
    /// A multiple selection shows its focused file, else the first in explorer order.
    fn resolve(&self) -> Resolved {
        if self.selected.is_empty() {
            return Resolved::default();
        }
        let index = self
            .focus
            .as_ref()
            .filter(|path| self.selected.contains(*path))
            .and_then(|path| self.files.iter().position(|file| &file.path == path))
            .or_else(|| self.files.iter().position(|file| self.selected.contains(&file.path)));
        let Some(index) = index else {
            return Resolved::default();
        };
        let file = self.files[index].clone();
        let kind = media_overlay::media_kind(&file);
        let same = |candidate: &&FileEntry| kind.is_some() && media_overlay::media_kind(candidate) == kind;
        Resolved {
            previous: self.files[..index].iter().rev().find(same).map(|file| file.path.clone()),
            next: self.files[index + 1..].iter().find(same).map(|file| file.path.clone()),
            file: Some(file),
        }
    }
    fn capture(owner: &OxydeApp, previous: &Source) -> Self {
        let from_tab = |tab: &crate::ui::workspace::TabState| Self {
            tab: tab.id.clone(),
            selected: if previous.tab == tab.id && *previous.selected == tab.selected_paths {
                previous.selected.clone()
            } else {
                Arc::new(tab.selected_paths.clone())
            },
            focus: tab.keyboard_cursor.clone().filter(|path| tab.selected_paths.contains(path))
                .or_else(|| tab.selection_anchor.clone()),
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
        options: (bool, bool, bool),
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&owner, |this, owner, cx| {
            let app = owner.read(cx);
            let source = Source::capture(app, &this.source);
            let options = (app.config.viewer_markdown_rendered, app.config.viewer_syntax_highlighting, app.config.viewer_media_autoplay);
            this.blocked = !this.detached && (app.settings_open || app.dragging_tab.is_some());
            this.set_source(source, cx);
            this.sync_display_options(options.0, options.1, cx);
            this.media_autoplay = options.2;
            this.update_visibility();
            cx.notify();
        });
        // Process startup dominates a first image; pay it while the user selects.
        cx.background_executor()
            .spawn(async { oxyde_lib::platform::platform().warm_viewer_images() })
            .detach();
        // The owner is being updated while it constructs this child. Read its
        // initial preferences at the call site, never through owner.read here.
        let (markdown_rendered, syntax_highlighting, media_autoplay) = options;
        let mut this = Self {
            owner: owner.downgrade(),
            tab_id,
            source: Source::default(),
            file: None,
            neighbors: (None, None),
            content: content::Content::None,
            markdown_rendered,
            syntax_highlighting,
            media_autoplay,
            generation: 0,
            wanted: Default::default(),
            loading: None,
            scroll: Default::default(),
            preparation: None,
            media: None,
            resume: None,
            external_open_pauses: HashSet::new(),
            seeking: false,
            seek_track: Bounds::default(),
            native_media: false,
            audio: false,
            detached: false,
            fullscreen_pending: false,
            fullscreen_temporary: false,
            image_view: Default::default(),
            fullscreen_controls: Default::default(),
            overlay: Default::default(),
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
        self.external_open_pauses.clear();
        self.preparation = None;
        if source.selected.is_empty() || source.files.len() <= 1024 {
            let resolved = source.resolve();
            self.source = source;
            self.publish(resolved, cx);
            return;
        }
        // A large directory scan must not block selection or playback controls.
        if !self.file.as_ref().is_some_and(|file| source.selected.contains(&file.path)) {
            self.file = None;
            self.neighbors = (None, None);
            self.load_current(cx);
        }
        self.source = source.clone();
        let expected = source.clone();
        let work = cx.background_executor().spawn(async move { source.resolve() });
        self.preparation = Some(cx.spawn(async move |this, cx| {
            let resolved = work.await;
            let _ = this.update(cx, |this, cx| {
                if !this.source.same_revision(&expected) {
                    return;
                }
                this.preparation = None;
                this.publish(resolved, cx);
                cx.notify();
            });
        }));
    }

    fn publish(&mut self, resolved: Resolved, cx: &mut Context<Self>) {
        let unchanged = self.file.as_ref().zip(resolved.file.as_ref()).is_some_and(|(old, file)| {
            old.path == file.path
                && old.content_stamp == file.content_stamp
                && old.modified == file.modified
                && old.size == file.size
        });
        self.file = resolved.file;
        self.neighbors = (resolved.previous, resolved.next);
        if !unchanged {
            self.load_current(cx);
        }
    }

    /// Skips to the neighboring media of the same kind by selecting it in the
    /// source pane; the Viewer then follows that selection.
    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let target = if forward { &self.neighbors.1 } else { &self.neighbors.0 };
        let (Some(path), Some(owner)) = (target.clone(), self.owner.upgrade()) else {
            return;
        };
        let tab = self.source.tab.clone();
        owner.update(cx, |app, cx| app.select_viewer_file(&tab, path, cx));
    }

    fn load_current(&mut self, cx: &mut Context<Self>) {
        self.close_fullscreen_controls(cx);
        // Overlay popups survive skipping, so the pointer can rest on an arrow.
        self.image_view = Default::default();
        self.generation += 1;
        self.wanted.store(self.generation, std::sync::atomic::Ordering::Release);
        self.loading = None;
        self.replace_content(content::Content::None, cx);
        self.media = None;
        self.resume = None;
        self.native_media = false;
        self.audio = false;
        let image = self.file.as_ref().is_some_and(|file| {
            !file.is_dir
                && file.archive_path.is_none()
                && crate::ui::components::icons::get_preview_type(&file.name)
                    == crate::ui::components::icons::PreviewType::Image
        });
        if !image {
            release_image_decoder(cx);
        }
        let Some(file) = self.file.as_ref() else {
            return;
        };
        if file.is_dir || file.archive_path.is_some() {
            return;
        }
        use crate::ui::components::icons::{AUDIO_EXTENSIONS, VIDEO_EXTENSIONS};
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
            if self.external_open_pauses.contains(&file.path) {
                self.resume = Some((0, true));
            }
            if self.audio {
                let path = file.path.clone();
                self.load_audio(path, cx);
            }
            if !cfg!(target_os = "windows") {
                self.native_media = false;
            }
            return;
        }
        let file = file.clone();
        self.load_content(&file, cx);
    }

    /// Only a detached window gives the Viewer keyboard focus. Docked, the
    /// explorer pane stays active and keeps the keyboard.
    fn activate(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.detached {
            window.focus(&self.focus, cx);
        }
    }
}

#[cfg(test)]
#[path = "viewer/tests/selection.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "viewer/tests/native_viewer.rs"]
mod native_tests;
