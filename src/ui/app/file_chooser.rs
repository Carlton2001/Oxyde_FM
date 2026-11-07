//! The window an application gets, through the desktop portal, to choose a file to
//! open. It browses real folders only and answers the application once: with the file
//! chosen, or nothing when the user cancels. Its state stays here; the main window
//! lends it its places, display choices and the folder the previous one ended in.
mod browse;
mod render;
mod setting;

use super::OxydeApp;
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
    entries: Arc<Vec<FileEntry>>,
    selected: Option<usize>,
    listing: Listing,
    places: Vec<Place>,
    show_hidden: bool,
    show_system: bool,
    system_icons: bool,
    list: UniformListScrollHandle,
    scroll: ScrollHandle,
    scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    grab_offset: f32,
    /// The scrollbar being dragged: its track, viewport and content heights.
    scroll_drag: Option<(String, f32, f32)>,
    focus: FocusHandle,
    listing_task: Option<Task<()>>,
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

/// Folders of the main window's sidebar a file can be in: places, then volumes.
fn places(app: &OxydeApp, i18n: &crate::ui::i18n::I18n) -> Vec<Place> {
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
    places.extend(app.state.drives.iter().filter(|drive| drive.path.starts_with('/')).map(|drive| Place {
        label: crate::ui::format::drive_name(drive, i18n),
        path: drive.path.clone(),
        icon: crate::ui::components::icons::drive_icon_path(drive),
    }));
    places
}

impl OxydeApp {
    /// Opens a window for an application's request; it starts where the last one ended.
    pub(crate) fn open_file_chooser(&mut self, request: FileChooserRequest, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let folder = self.file_chooser_folder.clone()
            .or_else(|| dirs::home_dir().map(|home| home.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "/".into());
        let places = places(self, crate::ui::i18n::I18nContext::i18n(cx));
        let options = (self.show_hidden, self.show_system, self.use_system_icons());
        let rem_size = self.ui_rem_size();
        // GPUI draws a new window before open_window returns, and its render reads the
        // owner: the window opens once this update of the owner has ended.
        cx.defer(move |cx| {
            let title = Some(request.title.clone()).filter(|title| !title.is_empty())
                .unwrap_or_else(|| crate::ui::i18n::I18nContext::i18n(cx).text(crate::ui::i18n::Message::FileChooserTitle).to_string());
            let default = Bounds::new(point(px(200.0), px(160.0)), size(px(crate::ui::scale::logical(880.0)), px(crate::ui::scale::logical(580.0))));
            let opened = cx.open_window(
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    window_decorations: crate::ui::window_decorations(),
                    window_bounds: Some(super::tool_geometry::restore(ICON, default, cx)),
                    window_min_size: Some(size(px(crate::ui::scale::logical(560.0)), px(crate::ui::scale::logical(380.0)))),
                    titlebar: Some(TitlebarOptions { appears_transparent: true, title: Some(title.into()), ..Default::default() }),
                    ..Default::default()
                },
                move |window, cx| {
                    window.set_rem_size(px(rem_size as f32));
                    cx.new(|cx| FileChooserWindow::new(owner, request, folder, places, options, window, cx))
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
    fn new(
        owner: WeakEntity<OxydeApp>,
        request: FileChooserRequest,
        folder: String,
        places: Vec<Place>,
        (show_hidden, show_system, system_icons): (bool, bool, bool),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let FileChooserRequest { title, accept_label, answer, withdrawn } = request;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        crate::ui::components::tool_window::install_icon(ICON, window);
        // Closed by the desktop rather than its buttons: the user cancels.
        let closing = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = closing.update(cx, |this, _| this.send(None));
            crate::ui::components::tool_window::close_transients(window, cx);
            true
        });
        // The application withdrew its request: the window goes, unanswered.
        let withdrawal = cx.spawn_in(window, async move |this, cx| {
            if withdrawn.await.is_ok() {
                let _ = this.update_in(cx, |_, window, _| window.remove_window());
            }
        });
        let list = UniformListScrollHandle::new();
        let scroll = list.0.borrow().base_handle.clone();
        let mut this = Self {
            owner,
            answer: Some(answer),
            title: Some(title).filter(|title| !title.is_empty()).map(SharedString::from),
            accept_label: accept_label.filter(|label| !label.is_empty()).map(SharedString::from),
            folder: String::new(),
            entries: Arc::new(Vec::new()),
            selected: None,
            listing: Listing::Loading,
            places,
            show_hidden,
            show_system,
            system_icons,
            list,
            scroll,
            scrollbar_origins: Default::default(),
            grab_offset: 0.0,
            scroll_drag: None,
            focus,
            listing_task: None,
            _withdrawal: withdrawal,
        };
        this.navigate(folder, cx);
        this
    }

    /// Answers the application, once.
    fn send(&mut self, chosen: Option<Vec<PathBuf>>) {
        let Some(answer) = self.answer.take() else { return };
        log::info!("file chooser answered: {chosen:?}");
        let _ = answer.send(chosen);
    }

    /// Answers and closes; the next window starts in this folder.
    fn finish(&mut self, chosen: Option<Vec<PathBuf>>, window: &mut Window, cx: &mut Context<Self>) {
        self.send(chosen);
        let folder = self.folder.clone();
        let _ = self.owner.update(cx, |owner, _| owner.file_chooser_folder = Some(folder));
        crate::ui::components::tool_window::close_transients(window, cx);
        window.remove_window();
    }
}
