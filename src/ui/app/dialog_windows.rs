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

#[derive(Clone, Copy)]
pub(super) enum DialogKind {
    Trash,
    Conflict,
}

impl DialogKind {
    fn index(self) -> usize {
        match self {
            Self::Trash => 0,
            Self::Conflict => 1,
        }
    }
    fn active(self, app: &OxydeApp) -> bool {
        match self {
            Self::Trash => app.trash_settings.is_some(),
            Self::Conflict => !app.pending_file_operations.is_empty(),
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
    _subscriptions: Vec<Subscription>,
}

pub(crate) fn close_dialog_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<DialogWindow>() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
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
            position,
            content: super::ContextMenuContent::Items(items, None),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 4.0,
            open_left: false,
            hovered_item: None,
            native_request_id: None,
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
        let origin = window.bounds().origin + point(px(60.0), px(60.0));
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
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        origin,
                        size(
                            px(rem
                                * match kind {
                                    DialogKind::Trash => 28.0,
                                    DialogKind::Conflict => 44.0,
                                }),
                            px(rem * 38.0),
                        ),
                    ))),
                    is_resizable: matches!(kind, DialogKind::Conflict),
                    window_min_size: matches!(kind, DialogKind::Conflict)
                        .then_some(size(px(rem * 40.0), px(rem * 12.0))),
                    is_minimizable: true,
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    window.set_rem_size(px(rem));
                    if let (Some(owner), Ok(handle)) = (
                        native_owner, raw_window_handle::HasWindowHandle::window_handle(window),
                    ) {
                        if let Err(error) = oxyde_lib::platform::platform().set_window_owner(handle.as_raw(), owner) {
                            log::warn!("Cannot attach recycle bin properties window to Oxyde: {error}");
                        }
                    }
                    tool_window::install_icon(
                        match kind {
                            DialogKind::Trash => "icons/trash-2.svg",
                            DialogKind::Conflict => "icons/copy-x.svg",
                        },
                        window,
                    );
                    let closing = close.clone();
                    window.on_window_should_close(cx, move |_, cx| {
                        let Some(main) = closing.upgrade() else {
                            return true;
                        };
                        main.update(cx, |app, cx| {
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
                            }
                            true
                        })
                    });
                    cx.new(|cx| {
                        let Some(main) = owner.upgrade() else {
                            unreachable!()
                        };
                        let observed = cx.observe(&main, |_, _, cx| cx.notify());
                        let released = cx.observe_release_in(&main, window, |_, _, window, _| {
                            window.remove_window()
                        });
                        let cleanup = owner.clone();
                        cx.on_release(move |_, cx| {
                            if let Some(main) = cleanup.upgrade() {
                                main.update(cx, |app, cx| {
                                    app.dialog_windows_open[kind.index()] = false;
                                    cx.notify();
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
                        });
                        DialogWindow {
                            owner,
                            kind,
                            advanced: false,
                            trash_request,
                            simple_size: None,
                            restoring_simple_width: None,
                            context_menus: Vec::new(),
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
                    }
                    app.file_operation_notice = Some((Message::OperationFailed, error.to_string()));
                    cx.notify();
                });
            }
        });
    }
}

impl Render for DialogWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(owner) = self.owner.upgrade() else {
            window.remove_window();
            return div().into_any_element();
        };
        if !self.kind.active(owner.read(cx)) {
            window.remove_window();
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
            window.remove_window();
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
        });
        let mut root = div()
            .size_full()
            .relative()
            .bg(cx.theme().active_background)
            .text_color(cx.theme().text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .child(content);
        for (index, menu) in self.context_menus.iter().enumerate() {
            let super::ContextMenuContent::Items(items, _) = &menu.content else {
                continue;
            };
            let handle = cx.entity();
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
            let position = menu_position(menu.position, menu_size, window.viewport_size());
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
                            .min(window.viewport_size().height.as_f32()),
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
        root.into_any_element()
    }
}
