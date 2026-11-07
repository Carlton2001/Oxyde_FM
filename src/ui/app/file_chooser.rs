//! The window an application gets, through the desktop portal, to choose a file to
//! open. It browses real folders only and answers the application once: with the file
//! chosen, or nothing when the user cancels. Its pieces are copies of the main window's
//! (navigation bar, columns, grid, status bar, sidebar rows), adapted to choosing; the
//! main window lends it its display choices, shares its places and volumes as they
//! change, and keeps its view, sort, sidebar width and last folder for the next one.
mod arrange;
mod browse;
mod crumbs;
mod details;
mod filter;
mod grid;
mod keys;
mod location;
mod navbar;
mod render;
mod setting;
mod sidebar;
mod status;

use super::OxydeApp;
use futures::channel::oneshot;
use gpui::*;
use oxyde_lib::models::{DriveInfo, FileEntry};
use oxyde_lib::platform::{FileChooserRequest, UnmountedVolume};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

const ICON: &str = "icons/folder-open.svg";
/// One tooltip session for the entries, as the main window's file list has.
const FILES_REGION: crate::ui::components::tooltip::hover::HoverRegion =
    crate::ui::components::tooltip::hover::HoverRegion::Controls("file-chooser-files");

/// A quick access folder the sidebar offers.
#[derive(Clone, PartialEq)]
struct Place {
    /// Empty for the home folder, which the sidebar names.
    label: String,
    path: String,
    home: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Listing {
    Loading,
    Ready,
    /// What the main window says of a folder it cannot list.
    Failed(crate::ui::i18n::Message),
}

/// Baseline widths of the metadata columns, measured as the main window measures them.
#[derive(Clone, Copy, Default)]
struct ColumnWidths {
    file_type: f32,
    size: f32,
    date: f32,
    permissions: f32,
}

pub struct FileChooserWindow {
    owner: WeakEntity<OxydeApp>,
    answer: Option<oneshot::Sender<Option<Vec<PathBuf>>>>,
    /// The application's title, and its label for the button that accepts.
    title: Option<SharedString>,
    accept_label: Option<SharedString>,
    folder: String,
    back: Vec<String>,
    forward: Vec<String>,
    entries: Arc<Vec<FileEntry>>,
    /// The entries the list shows, filtered and sorted, and how many are folders.
    shown: Arc<Vec<FileEntry>>,
    folders: usize,
    /// Type labels and sizes the metadata columns are measured from.
    column_values: Arc<(HashSet<String>, HashSet<(u64, bool, bool)>)>,
    column_revision: u64,
    column_widths: ColumnWidths,
    measured: Option<(u32, crate::ui::i18n::Language, crate::ui::format::DateFormat, u64)>,
    /// Folders holding folders, which their row marks with a chevron.
    subfolders: HashMap<String, bool>,
    /// A position in `shown`.
    selected: Option<usize>,
    listing: Listing,
    /// The main sidebar's places, followed while the window is open.
    places: Places,
    /// The volume being mounted from the sidebar.
    mounting_volume: Option<String>,
    options: arrange::Options,
    /// The sidebar's width at the baseline scale, and whether its edge is dragged.
    sidebar_width: f32,
    resizing_sidebar: bool,
    /// The window's width at the baseline scale, as last laid out.
    window_width: f32,
    show_hidden: bool,
    show_system: bool,
    show_permissions: bool,
    system_icons: bool,
    thumbnails: bool,
    media_previews: bool,
    /// The folder the tooltips were shown for: a new one dismisses them.
    tooltip_folder: String,
    filter: filter::Filter,
    location: location::Location,
    typed: super::type_select::TypeSelect,
    crumbs: crumbs::Crumbs,
    /// The details list; the grid scrolls its own handle.
    list: UniformListScrollHandle,
    grid_scroll: ScrollHandle,
    sidebar_scroll: ScrollHandle,
    /// The pane's width, as last laid out.
    pane_width: std::rc::Rc<std::cell::Cell<f32>>,
    scrollbar_origins: HashMap<String, Point<f32>>,
    grab_offset: f32,
    /// The scrollbar being dragged: its track, viewport and content heights.
    scroll_drag: Option<(String, f32, f32)>,
    focus: FocusHandle,
    listing_task: Option<Task<()>>,
    arrange_task: Option<Task<()>>,
    subfolder_task: Option<Task<()>>,
    _withdrawal: Task<()>,
    _owner_changes: Option<Subscription>,
}

impl crate::ui::components::scrollbar::ScrollbarState for FileChooserWindow {
    fn scrollbar_origins(&mut self) -> &mut HashMap<String, Point<f32>> {
        &mut self.scrollbar_origins
    }
    fn set_grab_offset(&mut self, offset: f32) {
        self.grab_offset = offset;
    }
}

/// Whether a file chooser waits for the user, which keeps Oxyde running.
pub(crate) fn is_open(cx: &App) -> bool {
    cx.windows().iter().any(|window| window.downcast::<FileChooserWindow>().is_some())
}

/// What the sidebar shows of the main one: its quick access folders a file can be in,
/// then the mounted drives and the volumes still to mount.
#[derive(Clone, Default, PartialEq)]
struct Places {
    folders: Vec<Place>,
    drives: Vec<DriveInfo>,
    volumes: Vec<UnmountedVolume>,
}

impl Places {
    fn of(app: &OxydeApp) -> Self {
        // As in Nautilus, the home folder heads the bookmarks without being one of them.
        let home = dirs::home_dir().map(|home| Place { label: String::new(), path: home.to_string_lossy().into_owned(), home: true });
        let pinned = app.state.pinned_items.iter()
            .filter(|item| item.path.starts_with('/'))
            .filter(|item| app.show_hidden || !item.name.starts_with('.'))
            .map(|item| Place { label: item.name.clone(), path: item.path.clone(), home: item.name.is_empty() });
        Self {
            folders: home.into_iter().chain(pinned).collect(),
            drives: app.state.drives.iter().filter(|drive| drive.path.starts_with('/')).cloned().collect(),
            volumes: app.state.unmounted_volumes.clone(),
        }
    }
}

/// What the window takes from the main one when it opens.
struct Lent {
    places: Places,
    options: arrange::Options,
    sidebar_width: f32,
    show_hidden: bool,
    show_system: bool,
    show_permissions: bool,
    system_icons: bool,
    thumbnails: bool,
    media_previews: bool,
}

impl OxydeApp {
    /// Opens a window for an application's request, where the last one ended.
    pub(crate) fn open_file_chooser(&mut self, request: FileChooserRequest, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let lent = Lent {
            places: Places::of(self),
            options: arrange::Options::from_value(self.config.file_chooser_options.as_ref()),
            sidebar_width: self.sidebar_width,
            show_hidden: self.show_hidden,
            show_system: self.show_system,
            show_permissions: self.show_permissions_column(),
            system_icons: cfg!(target_os = "windows") && self.use_system_icons(),
            thumbnails: self.show_grid_thumbnails,
            media_previews: self.show_media_previews,
        };
        let rem_size = self.ui_rem_size();
        // GPUI draws a new window before open_window returns, and its render reads the
        // owner: the window opens once this update of the owner has ended.
        cx.defer(move |cx| {
            let title = Some(request.title.clone()).filter(|title| !title.is_empty())
                .unwrap_or_else(|| crate::ui::i18n::I18nContext::i18n(cx).text(crate::ui::i18n::Message::FileChooserTitle).to_string());
            let logical = crate::ui::scale::logical;
            let default = Bounds::new(point(px(200.0), px(160.0)), size(px(logical(1040.0)), px(logical(640.0))));
            let opened = cx.open_window(
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    window_decorations: crate::ui::window_decorations(),
                    window_bounds: Some(super::tool_geometry::restore(ICON, default, cx)),
                    // The narrowest sidebar, its edge and the pane's minimum fit.
                    window_min_size: Some(size(px(logical(arrange::SIDEBAR_MIN + 1.0 + arrange::PANE_MIN)), px(logical(420.0)))),
                    titlebar: Some(TitlebarOptions { appears_transparent: true, title: Some(title.into()), ..Default::default() }),
                    ..Default::default()
                },
                move |window, cx| {
                    window.set_rem_size(px(rem_size as f32));
                    cx.new(|cx| FileChooserWindow::new(owner, request, lent, window, cx))
                },
            );
            if let Err(error) = opened {
                // The request drops with the window, which tells the application.
                log::error!("Cannot open the file chooser: {error}");
            }
        });
    }
}

impl FileChooserWindow {
    fn new(owner: WeakEntity<OxydeApp>, request: FileChooserRequest, lent: Lent, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let FileChooserRequest { title, accept_label, answer, withdrawn } = request;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        crate::ui::components::tool_window::install_icon(ICON, window);
        // Closed by the desktop rather than its buttons: the user cancels.
        let closing = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = closing.update(cx, |this, cx| {
                this.send(None);
                this.remember(cx);
            });
            crate::ui::components::tool_window::close_transients(window, cx);
            true
        });
        // The application withdrew its request: the window goes, unanswered.
        let withdrawal = cx.spawn_in(window, async move |this, cx| {
            if withdrawn.await.is_ok() {
                let _ = this.update_in(cx, |_, window, _| window.remove_window());
            }
        });
        // A volume mounted or plugged in, a folder pinned: the sidebar follows the main one.
        let owner_changes = owner.upgrade().map(|app| cx.observe(&app, |this: &mut Self, app, cx| {
            let places = Places::of(app.read(cx));
            if places != this.places {
                this.places = places;
                cx.notify();
            }
        }));
        let sidebar_width = lent.options.sidebar.unwrap_or(lent.sidebar_width);
        let folder = lent.options.folder.clone()
            .or_else(|| dirs::home_dir().map(|home| home.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "/".into());
        let mut this = Self {
            owner,
            answer: Some(answer),
            title: Some(title).filter(|title| !title.is_empty()).map(SharedString::from),
            accept_label: accept_label.filter(|label| !label.is_empty()).map(SharedString::from),
            folder: String::new(),
            back: Vec::new(),
            forward: Vec::new(),
            entries: Arc::new(Vec::new()),
            shown: Arc::new(Vec::new()),
            folders: 0,
            column_values: Default::default(),
            column_revision: 0,
            column_widths: ColumnWidths::default(),
            measured: None,
            subfolders: HashMap::new(),
            selected: None,
            listing: Listing::Loading,
            places: lent.places,
            mounting_volume: None,
            options: lent.options,
            sidebar_width,
            resizing_sidebar: false,
            window_width: 0.0,
            show_hidden: lent.show_hidden,
            show_system: lent.show_system,
            show_permissions: lent.show_permissions,
            system_icons: lent.system_icons,
            thumbnails: lent.thumbnails,
            media_previews: lent.media_previews,
            tooltip_folder: String::new(),
            filter: Default::default(),
            location: Default::default(),
            typed: Default::default(),
            crumbs: Default::default(),
            list: UniformListScrollHandle::new(),
            grid_scroll: ScrollHandle::new(),
            sidebar_scroll: ScrollHandle::new(),
            pane_width: Default::default(),
            scrollbar_origins: Default::default(),
            grab_offset: 0.0,
            scroll_drag: None,
            focus,
            listing_task: None,
            arrange_task: None,
            subfolder_task: None,
            _withdrawal: withdrawal,
            _owner_changes: owner_changes,
        };
        this.show_folder(folder, cx);
        this
    }

    /// Answers the application, once.
    fn send(&mut self, chosen: Option<Vec<PathBuf>>) {
        let Some(answer) = self.answer.take() else { return };
        log::info!("file chooser answered: {chosen:?}");
        let _ = answer.send(chosen);
    }

    /// Keeps the view, the sort, the sidebar's width and this folder for the next window.
    fn remember(&mut self, cx: &mut Context<Self>) {
        self.options.folder = Some(self.folder.clone());
        let options = self.options.to_value();
        let _ = self.owner.update(cx, |owner, cx| {
            if owner.config.file_chooser_options != options {
                owner.config.file_chooser_options = options;
                owner.save_preferences(cx);
            }
        });
    }

    /// Answers and closes.
    fn finish(&mut self, chosen: Option<Vec<PathBuf>>, window: &mut Window, cx: &mut Context<Self>) {
        self.send(chosen);
        self.remember(cx);
        crate::ui::components::tool_window::close_transients(window, cx);
        window.remove_window();
    }

    /// The main window, which the shared tooltips follow; none while a crumb's menu
    /// is open, as none shows over the main window's menus.
    fn tooltip_owner(&self) -> Option<Entity<OxydeApp>> {
        self.owner.upgrade().filter(|_| self.crumbs.menu.is_none())
    }

    /// The entry at a position of the list.
    fn shown_entry(&self, position: usize) -> Option<&FileEntry> {
        self.shown.get(position)
    }
}
