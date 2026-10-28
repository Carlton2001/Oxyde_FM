use super::OxydeApp;
use crate::ui::components::{conflict_dialog, tool_window};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

fn menu_position(
    position: Point<Pixels>,
    menu: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Point<Pixels> {
    point(
        position
            .x
            .clamp(px(0.0), (viewport.width - menu.width).max(px(0.0))),
        position
            .y
            .clamp(px(0.0), (viewport.height - menu.height).max(px(0.0))),
    )
}

#[cfg(test)]
#[path = "dialog_windows/tests/menu_position.rs"]
mod menu_position_tests;

mod fixed;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DialogKind {
    Trash,
    Conflict,
    Rename,
    Network,
    NetworkForce,
    RemoteLogin,
    TrashDelete,
    SettingsConfirmation,
    Notice,
}

pub(super) const DIALOG_KINDS: usize = 9;

impl DialogKind {
    pub(super) const FIXED: [Self; 7] = [
        Self::Rename,
        Self::Network,
        Self::NetworkForce,
        Self::RemoteLogin,
        Self::TrashDelete,
        Self::SettingsConfirmation,
        Self::Notice,
    ];

    fn index(self) -> usize {
        self as usize
    }
    fn active(self, app: &OxydeApp) -> bool {
        match self {
            Self::Trash => app.trash_settings.is_some(),
            Self::Conflict => !app.pending_file_operations.is_empty(),
            _ => self.fixed_active(app),
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Trash | Self::TrashDelete => "icons/trash-2.svg",
            Self::Conflict => "icons/copy-x.svg",
            Self::Rename => "icons/pencil.svg",
            Self::Network | Self::NetworkForce | Self::RemoteLogin => "icons/server.svg",
            Self::SettingsConfirmation => "icons/settings.svg",
            Self::Notice => "icons/circle-x.svg",
        }
    }
}

pub(super) struct DialogWindow {
    owner: WeakEntity<OxydeApp>,
    kind: DialogKind,
    advanced: bool,
    trash_request: Option<EntityId>,
    simple_size: Option<Size<Pixels>>,
    restoring_simple_width: Option<Pixels>,
    context_menus: Vec<super::ContextMenuInstance>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

pub(crate) fn close_dialog_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<DialogWindow>() {
            let _ = handle.update(cx, |_, window, cx| { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() });
        }
    }
}

pub(super) fn activate_trash_window(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<DialogWindow>() {
            if handle
                .read(cx)
                .is_ok_and(|view| matches!(view.kind, DialogKind::Trash))
            {
                let _ = handle.update(cx, |_, window, _| window.activate_window());
                return;
            }
        }
    }
}

pub(super) fn show_text_menu(
    items: Vec<crate::ui::components::dropdown::DropdownItem>,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(view) = window.root::<DialogWindow>().flatten() else {
        return false;
    };
    view.update(cx, |view, cx| {
        view.context_menus = vec![super::ContextMenuInstance {
            native_bounds: None,
            position,
            content: super::ContextMenuContent::Items(items, None),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 4.0,
            open_left: false,
            hovered_item: None,
            native_request_id: None, open_up: false,
        }];
        cx.notify();
    });
    true
}

impl OxydeApp {
    pub(super) fn ensure_dialog_window(
        &mut self,
        kind: DialogKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !kind.active(self) || self.dialog_windows_open[kind.index()] {
            return;
        }
        self.dialog_windows_open[kind.index()] = true;
        let owner = cx.entity().downgrade();
        let owner_bounds = window.bounds();
        let owner_window = window.window_handle();
        let origin = owner_bounds.origin + point(px(60.0), px(60.0));
        let native_owner = {
            raw_window_handle::HasWindowHandle::window_handle(window).ok().map(|handle| handle.as_raw())
        };
        let rem = self.ui_rem_size() as f32;
        cx.defer(move |cx| {
            let Some(main) = owner.upgrade() else {
                return;
            };
            if !kind.active(main.read(cx)) {
                main.update(cx, |app, _| app.dialog_windows_open[kind.index()] = false);
                return;
            }
            let close = owner.clone();
            let trash_request = main
                .read(cx)
                .trash_settings
                .as_ref()
                .map(|dialog| dialog.request_id());
            let initial_focus = kind.initial_focus(main.read(cx), cx);
            let restore_focus = main.read(cx).focus_handle.clone();
            let fixed_size = size(px(rem * kind.fixed_width()), px(rem * 10.0));
            let options = if kind.is_fixed() {
                // The native lock owns placement; start centered to avoid a jump.
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        owner_bounds.center() - point(fixed_size.width / 2.0, fixed_size.height / 2.0),
                        fixed_size,
                    ))),
                    is_resizable: false,
                    is_minimizable: false,
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    // Wayland and X11 parent the dialog at creation, above its owner.
                    #[cfg(target_os = "linux")]
                    kind: WindowKind::Floating,
                    ..Default::default()
                }
            } else {
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    window_bounds: Some(crate::ui::app::tool_geometry::restore(kind.icon(), Bounds::new(
                        origin,
                        size(
                            px(rem * if kind == DialogKind::Trash { 28.0 } else { 44.0 }),
                            px(rem * 38.0),
                        ),
                    ), cx)),
                    is_resizable: matches!(kind, DialogKind::Conflict),
                    window_min_size: matches!(kind, DialogKind::Conflict)
                        .then_some(size(px(rem * 40.0), px(rem * 12.0))),
                    is_minimizable: true,
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    #[cfg(target_os = "linux")]
                    kind: WindowKind::Floating,
                    ..Default::default()
                }
            };
            let result = cx.open_window(
                options,
                move |window, cx| {
                    window.set_rem_size(px(rem));
                    if let (Some(owner), Ok(handle)) = (
                        native_owner, raw_window_handle::HasWindowHandle::window_handle(window),
                    ) {
                        let platform = oxyde_lib::platform::platform();
                        let attached = if kind.is_fixed() {
                            platform.configure_fixed_dialog(handle.as_raw(), owner).or_else(|error| {
                                log::debug!("{error}");
                                platform.set_window_owner(handle.as_raw(), owner)
                            })
                        } else {
                            platform.set_window_owner(handle.as_raw(), owner)
                        };
                        if let Err(error) = attached {
                            log::warn!("Cannot attach dialog window to Oxyde: {error}");
                        }
                    }
                    tool_window::install_icon(kind.icon(), window);
                    let closing = close.clone();
                    window.on_window_should_close(cx, move |window, cx| {
                        let Some(main) = closing.upgrade() else {
                            crate::ui::components::tool_window::close_transients(window, cx);
                            return true;
                        };
                        let allowed = main.update(cx, |app, cx| {
                            match kind {
                                DialogKind::Trash => {
                                    if app.trash_settings.as_ref().map(|d| d.request_id())
                                        != trash_request
                                    {
                                        return true;
                                    }
                                    if app.trash_settings.as_ref().is_some_and(|d| d.busy) {
                                        return false;
                                    }
                                    app.trash_settings = None;
                                    cx.notify();
                                }
                                DialogKind::Conflict => app.cancel_pending_file_operation(cx),
                                _ => return kind.request_fixed_close(app, window, cx),
                            }
                            true
                        });
                        if allowed {
                            crate::ui::components::tool_window::close_transients(window, cx);
                        }
                        allowed
                    });
                    cx.new(|cx| {
                        let Some(main) = owner.upgrade() else {
                            unreachable!()
                        };
                        // Close from the notification, before a frame without content
                        // is presented: DWM would fade out that blank surface.
                        let observed = cx.observe_in(&main, window, |view: &mut DialogWindow, main, window, cx| {
                            let open = view.kind.active(main.read(cx))
                                && (view.kind != DialogKind::Trash
                                    || main.read(cx).trash_settings.as_ref().map(|d| d.request_id()) == view.trash_request);
                            if open {
                                cx.notify();
                            } else {
                                crate::ui::components::tool_window::close_transients(window, cx);
                                window.remove_window();
                            }
                        });
                        let released = cx.observe_release_in(&main, window, |_, _, window, cx| {
                            { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() }
                        });
                        let cleanup = owner.clone();
                        let dialog_focus = initial_focus.clone();
                        cx.on_release(move |_, cx| {
                            if let Some(main) = cleanup.upgrade() {
                                main.update(cx, |app, cx| {
                                    app.dialog_windows_open[kind.index()] = false;
                                    cx.notify();
                                });
                            }
                            if kind.is_fixed() {
                                // Dialog callbacks focus inside this window; give
                                // the owner its keyboard back when it kept a stale target.
                                cx.defer(move |cx| {
                                    let _ = owner_window.update(cx, |_, window, cx| {
                                        let stale = window.focused(cx).is_none_or(|focused| {
                                            dialog_focus.as_ref() == Some(&focused)
                                        });
                                        if stale {
                                            window.focus(&restore_focus, cx);
                                        }
                                    });
                                });
                            }
                        })
                        .detach();
                        main.update(cx, |app, cx| match kind {
                            DialogKind::Trash => {
                                if let Some(dialog) = &app.trash_settings {
                                    window.focus(&dialog.focus, cx);
                                }
                            }
                            DialogKind::Conflict => {
                                if let Some(pending) = app.pending_file_operations.front_mut() {
                                    pending.dialog.reset_focus();
                                }
                            }
                            _ => {}
                        });
                        let focus = cx.focus_handle();
                        if kind.is_fixed() {
                            window.focus(initial_focus.as_ref().unwrap_or(&focus), cx);
                        }
                        DialogWindow {
                            owner,
                            kind,
                            advanced: false,
                            trash_request,
                            simple_size: None,
                            restoring_simple_width: None,
                            context_menus: Vec::new(),
                            focus,
                            _subscriptions: vec![observed, released],
                        }
                    })
                },
            );
            if let Err(error) = result {
                log::error!("Cannot open independent dialog: {error}");
                main.update(cx, |app, cx| {
                    app.dialog_windows_open[kind.index()] = false;
                    match kind {
                        DialogKind::Trash => app.trash_settings = None,
                        DialogKind::Conflict => {
                            app.pending_file_operations.pop_front();
                        }
                        _ => kind.discard_fixed(app, cx),
                    }
                    if kind != DialogKind::Notice {
                        app.file_operation_notice = Some((Message::OperationFailed, error.to_string()));
                    }
                    cx.notify();
                });
            }
        });
    }
}

impl Render for DialogWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(owner) = self.owner.upgrade() else {
            { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() };
            return div().into_any_element();
        };
        if !self.kind.active(owner.read(cx)) {
            { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() };
            return div().into_any_element();
        }
        if matches!(self.kind, DialogKind::Trash)
            && owner
                .read(cx)
                .trash_settings
                .as_ref()
                .map(|d| d.request_id())
                != self.trash_request
        {
            { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() };
            return div().into_any_element();
        }
        let advanced = matches!(self.kind, DialogKind::Conflict)
            && owner
                .read(cx)
                .pending_file_operations
                .front()
                .is_some_and(|p| p.dialog.is_advanced());
        window.set_rem_size(px(owner.read(cx).ui_rem_size() as f32));
        if advanced != self.advanced {
            let requested_size = if advanced {
                self.simple_size = Some(window.viewport_size());
                size(window.rem_size() * 62.0, window.rem_size() * 42.0)
            } else {
                let restored = self.simple_size.unwrap_or(window.viewport_size());
                self.restoring_simple_width = Some(restored.width);
                restored
            };
            window.on_next_frame(move |window, cx| {
                if window
                    .root::<DialogWindow>()
                    .flatten()
                    .is_none_or(|view| view.read(cx).advanced != advanced)
                {
                    return;
                }
                if window.is_maximized() {
                    window.zoom_window();
                }
                window.resize(requested_size);
            });
            self.advanced = advanced;
        }
        if matches!(self.kind, DialogKind::Trash) {
            window.set_window_title(cx.i18n().text(Message::TrashProperties).as_ref());
        }
        let kind = self.kind;
        if self.restoring_simple_width.is_some_and(|width|
            (window.viewport_size().width - width).abs() <= px(1.0)) {
            self.restoring_simple_width = None;
        }
        let simple_width = self.restoring_simple_width;
        if kind.is_fixed() && window.focused(cx).is_none() {
            let focus = kind.initial_focus(owner.read(cx), cx).unwrap_or_else(|| self.focus.clone());
            window.focus(&focus, cx);
        }
        let focus = self.focus.clone();
        let content = owner.update(cx, |app, cx| match kind {
            DialogKind::Trash => {
                super::trash_settings::render(app, owner.clone(), window, cx).into_any_element()
            }
            DialogKind::Conflict => {
                let system = app.use_system_icons();
                let pending = app.pending_file_operations.front_mut().unwrap();
                conflict_dialog::render_conflict_dialog(pending, system, simple_width, window, owner.clone(), cx)
                    .into_any_element()
            }
            _ => kind.render_fixed(app, owner.clone(), &focus, window, cx),
        });
        let root = div()
            .size_full()
            .relative()
            .bg(cx.theme().active_background)
            .text_color(cx.theme().text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .child(content);
                let mut root = root;
        for (index, menu) in self.context_menus.iter().enumerate() {
            let super::ContextMenuContent::Items(items, _) = &menu.content else {
                continue;
            };
            let handle = cx.entity();
            #[cfg(target_os = "windows")]
            if crate::ui::components::popup::supported(window, cx) {
                let scroll = menu.scroll_handle.clone();
                crate::ui::components::popup::text_menu(
                    format!("dialog-text-menu-{index}"), crate::ui::components::popup::PopupAnchor::point(menu.position, false),
                    handle, owner.clone(), items.clone(), scroll.clone(), window, cx,
                    move |view| view.context_menus.get(index).is_some_and(|menu| std::rc::Rc::ptr_eq(&menu.scroll_handle.0, &scroll.0)),
                    |view, _, cx| { view.context_menus.clear(); cx.notify(); },
                );
                continue;
            }
                        {
            let close = handle.clone();
            let measured = handle.clone();
            let menu_size = size(
                if menu.width > px(0.0) {
                    menu.width
                } else {
                    crate::ui::scale::rem_pixels(rems(13.0))
                },
                crate::ui::scale::rem_pixels(rems(items.len() as f32 * 1.625 + 1.0)),
            );
            // The root starts after the Linux shadow margin; menu positions are window points.
            let shown = crate::ui::components::client_decorations::content_bounds(window);
            let position = menu_position(menu.position - shown.origin, menu_size, shown.size);
            root = root.child(deferred(
                div().absolute().left(position.x).top(position.y).child(
                    crate::ui::components::dropdown::GenericDropdown {
                        id: ("dialog-text-menu", index).into(),
                        items: items.clone(),
                        reserve_icon_space: true,
                        is_context_menu: true,
                        bottom_padding: 4.0,
                        scroll_handle: menu.scroll_handle.clone(),
                        h_app: owner.clone(),
                        on_drag: std::rc::Rc::new(|_, _, _, _, _| {}),
                        on_close: std::rc::Rc::new(move |_, cx| {
                            close.update(cx, |view, cx| {
                                view.context_menus.clear();
                                cx.notify();
                            })
                        }),
                        on_outside: None,
                        width: None,
                        max_height: crate::ui::scale::logical(480.0)
                            .min(shown.size.height.as_f32()),
                        is_dragging: false,
                        header: None,
                        on_item_hover: None,
                        on_width_calculated: Some(std::rc::Rc::new(move |width, cx| {
                            measured.update(cx, |view, cx| {
                                if let Some(menu) = view.context_menus.get_mut(index) {
                                    if menu.width != width {
                                        menu.width = width;
                                        cx.notify();
                                    }
                                }
                            });
                        })),
                        on_hover_change: None,
                    },
                ),
            ));
            }
        }
        crate::ui::components::client_decorations::frame(root, cx.theme().border, window)
    }
}
