use super::*;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;

#[derive(Default)]
struct GhostWindow {
    token: Option<EntityId>,
    handle: Option<WindowHandle<Ghost>>,
}
impl Global for GhostWindow {}

struct Ghost {
    app: WeakEntity<OxydeApp>,
    paths: Vec<String>,
    is_dir: bool,
    measured: Option<Size<Pixels>>,
    native: bool,
    _subscription: Subscription,
}

pub(super) fn is_open(cx: &App) -> bool {
    cx.try_global::<GhostWindow>()
        .is_some_and(|state| state.handle.is_some())
}

pub(super) fn close(cx: &mut App) {
    let state = cx.default_global::<GhostWindow>();
    state.token = None;
    if let Some(handle) = state.handle.take() {
        let _ = handle.update(cx, |_, window, _| {
            if let Ok(raw) = HasWindowHandle::window_handle(window) {
                if let Err(error) = platform().dismiss_drag_window(raw.as_raw()) {
                    log::warn!("Cannot dismiss drag ghost: {error}");
                }
            }
        });
        // Closing a native window can synchronously enter GPUI. Defer it.
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        });
    }
}

pub(super) fn open(
    token: EntityId,
    paths: Vec<String>,
    is_dir: bool,
    app: WeakEntity<OxydeApp>,
    parent: &Window,
    cx: &mut App,
) {
    close(cx);
    cx.default_global::<GhostWindow>().token = Some(token);
    let owner = HasWindowHandle::window_handle(parent)
        .ok()
        .map(|raw| raw.as_raw());
    let origin = parent.bounds().origin;
    let rem = parent.rem_size();
    let parent = Window::window_handle(parent);
    cx.defer(move |cx| {
        if cx.default_global::<GhostWindow>().token != Some(token) {
            return;
        }
        let Some(owner) = owner else {
            return;
        };
        let Some(entity) = app.upgrade() else {
            return;
        };
        let result = cx.open_window(
            WindowOptions {
                kind: WindowKind::PopUp,
                titlebar: None,
                focus: false,
                show: false,
                window_background: WindowBackgroundAppearance::Transparent,
                is_resizable: false,
                is_minimizable: false,
                is_movable: false,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    origin,
                    size(rem * 28.0, rem * 5.0),
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_rem_size(rem);
                cx.new(|cx| Ghost {
                    app,
                    paths,
                    is_dir,
                    measured: None,
                    native: false,
                    _subscription: cx.observe(&entity, |_, _, cx| cx.notify()),
                })
            },
        );
        match result {
            Ok(handle) => {
                let configured = handle
                    .update(cx, |_, window, _| {
                        let raw = HasWindowHandle::window_handle(window)
                            .map_err(|error| error.to_string())?;
                        platform().configure_hover_window(raw.as_raw(), owner)
                    })
                    .ok()
                    .and_then(Result::err);
                if let Some(error) = configured {
                    log::warn!("Cannot configure drag ghost: {error}");
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                } else {
                    cx.default_global::<GhostWindow>().handle = Some(handle);
                    let _ = parent.update(cx, |_, window, cx| {
                        window.refresh();
                        sync(false, window, cx);
                    });
                }
            }
            Err(error) => log::warn!("Cannot create native drag ghost: {error}"),
        }
    });
}

pub(super) fn sync(native: bool, parent: &Window, cx: &mut App) {
    let Some(handle) = cx.default_global::<GhostWindow>().handle else {
        return;
    };
    let rem = parent.rem_size();
    let _ = handle.update(cx, |ghost, window, cx| {
        ghost.native = native;
        if window.rem_size() != rem {
            window.set_rem_size(rem);
        }
        place(ghost, window);
        cx.notify();
    });
}

fn place(ghost: &Ghost, window: &Window) {
    if let (Some(measured), Ok(raw)) = (ghost.measured, HasWindowHandle::window_handle(window)) {
        let _ = platform().place_drag_window(
            raw.as_raw(),
            [measured.width.as_f32(), measured.height.as_f32()],
            window.scale_factor(),
            ghost.native,
        );
    }
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (copy, new_tab, unavailable) = self
            .app
            .upgrade()
            .map(|app| {
                let app = app.read(cx);
                let native = app
                    .file_drag_session
                    .as_ref()
                    .filter(|session| session.native);
                let blocked = app
                    .file_drag_session
                    .as_ref()
                    .is_some_and(|session| session.outside && !session.ready);
                (
                    native.map_or(app.drag_copy_mode, |session| {
                        session.effect == FileDropEffect::Copy
                    }),
                    native.is_none() && app.drag_new_tab,
                    blocked || native.is_some_and(|session| session.effect == FileDropEffect::None),
                )
            })
            .unwrap_or((false, false, true));
        let content =
            super::super::ghost_content(&self.paths, self.is_dir, copy, new_tab, unavailable, cx)
                .font_weight(FontWeight::NORMAL)
                .text_color(cx.theme().text)
                .into_any_element();
        MeasuredGhost {
            content,
            owner: cx.entity(),
        }
    }
}

struct MeasuredGhost {
    content: AnyElement,
    owner: Entity<Ghost>,
}
impl IntoElement for MeasuredGhost {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for MeasuredGhost {
    type RequestLayoutState = Size<Pixels>;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Size<Pixels>) {
        let measured = self
            .content
            .layout_as_root(AvailableSpace::min_size(), window, cx);
        let style = Style {
            size: size(measured.width.into(), measured.height.into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), measured)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        measured: &mut Size<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.prepaint_at(Point::default(), window, cx);
        if self.owner.read(cx).measured != Some(*measured) {
            let owner = self.owner.clone();
            let measured = *measured;
            let handle = Window::window_handle(window);
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    owner.update(cx, |ghost, _| {
                        ghost.measured = Some(measured);
                        place(ghost, window);
                    })
                });
            });
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Size<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}
