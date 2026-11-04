//! Theme window: built-in and user themes applied on selection, and the editor of a user theme.
//! The selection and saved themes stay in `OxydeApp`; this window owns only the draft.

mod draft;
mod editor;
mod list;
mod render;

use super::OxydeApp;
use crate::ui::elements::color_picker::MeasuredBounds;
use crate::ui::i18n::{I18nContext, Message};
use draft::ThemeDraft;
use gpui::*;

const ICON: &str = "icons/palette.svg";

pub(crate) fn close_theme_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<ThemeWindow>() {
            let _ = handle.update(cx, |_, window, cx| {
                crate::ui::components::tool_window::close_transients(window, cx);
                window.remove_window()
            });
        }
    }
}

/// Part of the color picker following the pointer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PickerPart {
    Field,
    Hue,
}

pub struct ThemeWindow {
    owner: WeakEntity<OxydeApp>,
    draft: Option<ThemeDraft>,
    focus: FocusHandle,
    field_bounds: MeasuredBounds,
    hue_bounds: MeasuredBounds,
    drag: Option<PickerPart>,
    _subscriptions: Vec<Subscription>,
}

impl OxydeApp {
    pub(crate) fn open_theme_window(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.context_menus.clear();
        self.settings_open = false;
        self.settings_picker = None;
        self.settings_submenu_bounds = None;
        cx.notify();
        if let Some(handle) = cx.windows().into_iter().find_map(|handle| handle.downcast::<ThemeWindow>()) {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.activate_window());
            });
            return;
        }
        let owner = cx.entity().downgrade();
        let title = cx.i18n().text(Message::ThemeWindowTitle);
        // A new window draws synchronously; defer until the owner update ends.
        cx.defer(move |cx| {
            if owner.upgrade().is_none() {
                return;
            }
            let result = cx.open_window(
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    window_decorations: crate::ui::window_decorations(),
                    window_bounds: Some(crate::ui::app::tool_geometry::restore(ICON, Bounds::new(
                        point(px(180.0), px(120.0)),
                        size(px(900.0), px(600.0)),
                    ), cx)),
                    is_resizable: false,
                    is_minimizable: false,
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some(title),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| ThemeWindow::new(owner, window, cx)),
            );
            if let Err(error) = result {
                log::error!("Cannot open theme window: {error}");
            }
        });
    }
}

impl ThemeWindow {
    fn new(owner: WeakEntity<OxydeApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let closing = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = closing.update(cx, |this, cx| this.discard_draft(cx));
            crate::ui::components::tool_window::close_transients(window, cx);
            true
        });
        crate::ui::components::tool_window::install_icon(ICON, window);
        // A selected user theme opens in its editor, as when chosen in the list.
        cx.defer_in(window, |this: &mut Self, window, cx| {
            let custom = this.owner.upgrade()
                .is_some_and(|owner| matches!(owner.read(cx).theme_choice, crate::ui::theme::ThemeChoice::Custom(_)));
            if custom {
                this.start_draft(true, window, cx);
            }
        });
        let mut subscriptions = Vec::new();
        if let Some(main) = owner.upgrade() {
            subscriptions.push(cx.observe_release_in(&main, window, |_, _, window, cx| {
                crate::ui::components::tool_window::close_transients(window, cx);
                window.remove_window()
            }));
        }
        Self {
            owner,
            draft: None,
            focus,
            field_bounds: Default::default(),
            hue_bounds: Default::default(),
            drag: None,
            _subscriptions: subscriptions,
        }
    }

    /// Closing drops an unsaved draft and shows the saved theme again.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.discard_draft(cx);
        crate::ui::components::tool_window::close_transients(window, cx);
        window.remove_window();
    }
}
