//! The window an application gets, through the desktop portal, to choose a file to
//! open. It browses real folders only and answers the application once: with the file
//! chosen, or nothing when the user cancels. Its state stays here; the main window
//! lends it its places, volumes and display choices, and keeps its view, sort and last
//! folder for the next one.
mod arrange;
mod browse;
mod keys;
mod location;
mod render;
mod setting;
mod toolbar;

use super::OxydeApp;
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::i18n::I18nContext;
use futures::channel::oneshot;
use gpui::*;
use oxyde_lib::models::FileEntry;
use oxyde_lib::platform::FileChooserRequest;
use std::path::PathBuf;
use std::sync::Arc;

const ICON: &str = "icons/folder-open.svg";

/// A folder the window offers beside the list.
struct Place {
    label: String,
    path: String,
    icon: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Listing {
    Loading,
    Ready,
    Failed,
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
    /// Positions in `entries` the list shows, in its order.
    shown: Arc<Vec<usize>>,
    /// A position in `shown`.
    selected: Option<usize>,
    listing: Listing,
    places: Vec<Place>,
    volumes: Vec<Place>,
    options: arrange::Options,
    show_hidden: bool,
    show_system: bool,
    system_icons: bool,
    thumbnails: bool,
    filter: Entity<TextInput>,
    filter_text: String,
    /// The path being typed in place of the folder's crumbs.
    location: Option<Entity<TextInput>>,
    location_missing: bool,
    /// Letters typed in the list, and when the last one came.
    typed: (String, std::time::Instant),
    /// Cards per row of the grid, and the list's width they fill, as last laid out.
    columns: usize,
    grid_width: std::rc::Rc<std::cell::Cell<f32>>,
    /// The folder's crumbs, scrolled to the last.
    crumbs: ScrollHandle,
    list: UniformListScrollHandle,
    scroll: ScrollHandle,
    scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    grab_offset: f32,
    /// The scrollbar being dragged: its track, viewport and content heights.
    scroll_drag: Option<(String, f32, f32)>,
    focus: FocusHandle,
    listing_task: Option<Task<()>>,
    arrange_task: Option<Task<()>>,
    location_task: Option<Task<()>>,
    _withdrawal: Task<()>,
}

impl crate::ui::components::scrollbar::ScrollbarState for FileChooserWindow {
    fn scrollbar_origins(&mut self) -> &mut std::collections::HashMap<String, Point<f32>> {
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

/// The main window's places a file can be in, then its volumes.
fn places(app: &OxydeApp, i18n: &crate::ui::i18n::I18n) -> (Vec<Place>, Vec<Place>) {
    use crate::ui::i18n::Message;
    let home = dirs::home_dir().map(|home| home.to_string_lossy().into_owned());
    let mut places: Vec<Place> = app.state.pinned_items.iter()
        .filter(|item| item.path.starts_with('/'))
        .filter(|item| app.show_hidden || !item.name.starts_with('.'))
        .map(|item| {
            let is_home = home.as_deref() == Some(item.path.as_str());
            Place {
                label: if item.name.is_empty() || is_home { i18n.text(Message::SidebarHome).to_string() } else { item.name.clone() },
                path: item.path.clone(),
                icon: if is_home { "icons/house.svg" } else { "icons/pin.svg" },
            }
        })
        .collect();
    if let Some(home) = home.filter(|home| !places.iter().any(|place| place.path == *home)) {
        places.insert(0, Place { label: i18n.text(Message::SidebarHome).to_string(), path: home, icon: "icons/house.svg" });
    }
    let volumes = app.state.drives.iter().filter(|drive| drive.path.starts_with('/')).map(|drive| Place {
        label: crate::ui::format::drive_name(drive, i18n),
        path: drive.path.clone(),
        icon: crate::ui::components::icons::drive_icon_path(drive),
    }).collect();
    (places, volumes)
}

/// What the window takes from the main one when it opens.
struct Lent {
    places: (Vec<Place>, Vec<Place>),
    options: arrange::Options,
    show_hidden: bool,
    show_system: bool,
    system_icons: bool,
    thumbnails: bool,
}

impl OxydeApp {
    /// Opens a window for an application's request, where the last one ended.
    pub(crate) fn open_file_chooser(&mut self, request: FileChooserRequest, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let lent = Lent {
            places: places(self, cx.i18n()),
            options: arrange::Options::from_value(self.config.file_chooser_options.as_ref()),
            show_hidden: self.show_hidden,
            show_system: self.show_system,
            system_icons: self.use_system_icons(),
            thumbnails: self.show_grid_thumbnails,
        };
        let rem_size = self.ui_rem_size();
        // GPUI draws a new window before open_window returns, and its render reads the
        // owner: the window opens once this update of the owner has ended.
        cx.defer(move |cx| {
            let title = Some(request.title.clone()).filter(|title| !title.is_empty())
                .unwrap_or_else(|| cx.i18n().text(crate::ui::i18n::Message::FileChooserTitle).to_string());
            let default = Bounds::new(point(px(200.0), px(160.0)), size(px(crate::ui::scale::logical(960.0)), px(crate::ui::scale::logical(620.0))));
            let opened = cx.open_window(
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    window_decorations: crate::ui::window_decorations(),
                    window_bounds: Some(super::tool_geometry::restore(ICON, default, cx)),
                    window_min_size: Some(size(px(crate::ui::scale::logical(640.0)), px(crate::ui::scale::logical(400.0)))),
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
        let filter = Self::filter_input(cx.entity().downgrade(), window, cx);
        let list = UniformListScrollHandle::new();
        let scroll = list.0.borrow().base_handle.clone();
        let folder = lent.options.folder.clone()
            .or_else(|| dirs::home_dir().map(|home| home.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "/".into());
        let (places, volumes) = lent.places;
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
            selected: None,
            listing: Listing::Loading,
            places,
            volumes,
            options: lent.options,
            show_hidden: lent.show_hidden,
            show_system: lent.show_system,
            system_icons: lent.system_icons,
            thumbnails: lent.thumbnails,
            filter,
            filter_text: String::new(),
            location: None,
            location_missing: false,
            typed: (String::new(), std::time::Instant::now()),
            columns: 1,
            grid_width: Default::default(),
            crumbs: ScrollHandle::new(),
            list,
            scroll,
            scrollbar_origins: Default::default(),
            grab_offset: 0.0,
            scroll_drag: None,
            focus,
            listing_task: None,
            arrange_task: None,
            location_task: None,
            _withdrawal: withdrawal,
        };
        this.show_folder(folder, cx);
        this
    }

    fn filter_input(this: WeakEntity<Self>, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextInput> {
        let placeholder = cx.i18n().text(crate::ui::i18n::Message::NameFilterPlaceholder);
        let (changed, entered, escaped) = (this.clone(), this.clone(), this);
        cx.new(|cx| TextInput::new(
            cx,
            placeholder,
            String::new(),
            move |value, _, cx| { let _ = changed.update(cx, |this, cx| this.set_filter(value, cx)); },
            move |window, cx| { let _ = entered.update(cx, |this, cx| this.accept(window, cx)); },
            move |window, cx| { let _ = escaped.update(cx, |this, cx| this.clear_filter(window, cx)); },
            |_, _, _, _, _| {},
            TextInputOptions { appearance: InputAppearance::Embedded, clearable: true, ..Default::default() },
            window,
        ))
    }

    /// Answers the application, once.
    fn send(&mut self, chosen: Option<Vec<PathBuf>>) {
        let Some(answer) = self.answer.take() else { return };
        log::info!("file chooser answered: {chosen:?}");
        let _ = answer.send(chosen);
    }

    /// Keeps the view, the sort and this folder for the next window.
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

    /// The entry at a position of the list.
    fn shown_entry(&self, position: usize) -> Option<&FileEntry> {
        self.entries.get(*self.shown.get(position)?)
    }
}
