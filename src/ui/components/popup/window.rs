mod frame;
use super::PopupAnchor;
use frame::FramedContent;
use gpui::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{collections::HashMap, rc::Rc, time::Duration};

type Key = (WindowId, SharedString);
type Factory = Rc<dyn Fn(&mut Window, &mut App) -> Option<AnyElement>>;
type Alive = Rc<dyn Fn(&App) -> bool>;

#[derive(Clone)]
struct Spec {
    key: Key,
    parent: AnyWindowHandle,
    raw_owner: RawWindowHandle,
    #[cfg(target_os = "windows")]
    raw_parent: RawWindowHandle,
    restore_parent: AnyWindowHandle,
    anchor: PopupAnchor,
    rem: Pixels,
    restore: Option<FocusHandle>,
    interactive: bool,
    modifiers: Modifiers,
    factory: Factory,
    alive: Alive,
    close: Rc<dyn Fn(&mut App)>,
}

#[derive(Default)]
struct Surfaces {
    windows: HashMap<Key, WindowHandle<MenuWindow>>,
    owners: HashMap<WindowId, (AnyWindowHandle, RawWindowHandle)>,
    pending: HashMap<Key, Spec>,
    failed: HashMap<WindowId, EntityId>,
    cleanup: Option<Subscription>,
    logical_owners: HashMap<WindowId, AnyWindowHandle>,
    raw_windows: HashMap<WindowId, RawWindowHandle>,
    rows: HashMap<WindowId, HashMap<usize, Bounds<Pixels>>>,
    previous_rows: HashMap<WindowId, HashMap<usize, Bounds<Pixels>>>,
    #[cfg(not(target_os = "windows"))]
    origins: HashMap<WindowId, Rc<std::cell::Cell<Point<Pixels>>>>,
    #[cfg(not(target_os = "windows"))]
    dismissals: HashMap<WindowId, (Alive, Rc<dyn Fn(&mut App)>)>,
}
impl Global for Surfaces {}

/// The compositor places these surfaces and reports where, relative to their parent.
#[cfg(not(target_os = "windows"))]
fn owner_origin(state: &Surfaces, mut id: WindowId) -> Point<Pixels> {
    let mut origin = Point::default();
    while let Some(offset) = state.origins.get(&id) {
        origin = origin + offset.get();
        let Some(parent) = state.logical_owners.get(&id) else { break };
        id = parent.window_id();
    }
    origin
}

/// Only the topmost menu may take a compositor grab, and asking otherwise is a fatal protocol
/// error: close the menus that the new one does not descend from.
#[cfg(not(target_os = "windows"))]
fn close_unrelated(parent: AnyWindowHandle, cx: &mut App) {
    let state = cx.default_global::<Surfaces>();
    let mut ancestors = vec![parent.window_id()];
    while let Some(next) = state.logical_owners.get(ancestors.last().unwrap()) {
        ancestors.push(next.window_id());
    }
    let keys: Vec<_> = state.windows.iter()
        .filter(|(_, handle)| !ancestors.contains(&handle.window_id()))
        .map(|(key, _)| key.clone())
        .collect();
    for key in keys {
        let Some(&handle) = cx.default_global::<Surfaces>().windows.get(&key) else { continue };
        let Ok(spec) = handle.read(cx).map(|root| root.spec.clone()) else { continue };
        if !spec.interactive {
            continue;
        }
        cx.default_global::<Surfaces>().windows.remove(&key);
        let _ = handle.update(cx, |_, window, cx| {
            close_for(window, cx);
            crate::ui::components::tooltip::hover::close_for(window, cx);
            window.remove_window();
        });
        if (spec.alive)(cx) {
            (spec.close)(cx);
        }
    }
}

pub(crate) fn supported(window: &Window, cx: &App) -> bool {
    #[cfg(not(target_os = "windows"))]
    if !super::available() {
        return false;
    }
    let parent = owner(window, cx).unwrap_or(Window::window_handle(window));
    !cx.try_global::<Surfaces>()
        .is_some_and(|state| state.failed.contains_key(&parent.window_id()))
}

pub(crate) fn supported_source<T: 'static>(source: &Entity<T>, cx: &App) -> bool {
    #[cfg(not(target_os = "windows"))]
    if !super::available() {
        return false;
    }
    !cx.try_global::<Surfaces>()
        .is_some_and(|state| state.failed.values().any(|id| *id == source.entity_id()))
}

fn failed(parent: AnyWindowHandle, source: EntityId, cx: &mut App) {
    let state = cx.default_global::<Surfaces>();
    state.failed.insert(parent.window_id(), source);
    let keys: Vec<_> = state
        .windows
        .keys()
        .filter(|key| key.0 == parent.window_id())
        .cloned()
        .collect();
    let handles: Vec<_> = keys
        .into_iter()
        .filter_map(|key| state.windows.remove(&key))
        .collect();
    for handle in handles {
        let _ = handle.update(cx, |_, window, cx| {
            crate::ui::components::tooltip::hover::close_for(window, cx);
            window.remove_window();
        });
    }
    let _ = parent.update(cx, |_, window, _| window.refresh());
}

pub(crate) fn surface(
    parent: AnyWindowHandle,
    id: SharedString,
    cx: &App,
) -> Option<AnyWindowHandle> {
    cx.try_global::<Surfaces>()?
        .windows
        .get(&(parent.window_id(), id))
        .map(|handle| (*handle).into())
}
pub(crate) fn record_row(window: &Window, cx: &mut App, index: usize, bounds: Bounds<Pixels>) {
    let id = window.window_handle().window_id();
    if owner(window, cx).is_some() {
        cx.default_global::<Surfaces>()
            .rows
            .entry(id)
            .or_default()
            .insert(index, bounds);
    }
}
pub(crate) fn row_bounds(
    parent: AnyWindowHandle,
    key: SharedString,
    index: usize,
    cx: &App,
) -> Option<Bounds<Pixels>> {
    let state = cx.try_global::<Surfaces>()?;
    let id = state.windows.get(&(parent.window_id(), key))?.window_id();
    let bounds = *state.rows.get(&id)?.get(&index)?;
    #[cfg(not(target_os = "windows"))]
    return Some(bounds + owner_origin(state, id));
    #[cfg(target_os = "windows")]
    let raw = *state.raw_windows.get(&id)?;
    #[cfg(target_os = "windows")]
    let coordinate_owner = state.owners.get(&id)?.1;
    #[cfg(target_os = "windows")]
    let platform = oxyde_lib::platform::platform();
    #[cfg(target_os = "windows")]
    {
        let (x, y) = platform
            .popup_point_to_owner(
                raw,
                coordinate_owner,
                (bounds.left().as_f32(), bounds.top().as_f32()),
            )
            .ok()?;
        let (right, bottom) = platform
            .popup_point_to_owner(
                raw,
                coordinate_owner,
                (bounds.right().as_f32(), bounds.bottom().as_f32()),
            )
            .ok()?;
        Some(Bounds::new(
            point(px(x), px(y)),
            size(px(right - x), px(bottom - y)),
        ))
    }
}

struct MenuWindow {
    spec: Spec,
    measured: Option<Size<Pixels>>,
    focus: FocusHandle,
    // The first frame is empty: Wayland has no hidden surface, and this one needs a buffer
    // before it can take the size of its content.
    #[cfg(not(target_os = "windows"))]
    ready: bool,
    #[cfg(not(target_os = "windows"))]
    placed: std::cell::Cell<Option<(Bounds<Pixels>, Size<Pixels>)>>,
    _observer: Subscription,
    _watcher: Task<()>,
}

pub(crate) fn owner(window: &Window, cx: &App) -> Option<AnyWindowHandle> {
    cx.try_global::<Surfaces>()?
        .owners
        .get(&Window::window_handle(window).window_id())
        .map(|entry| entry.0)
}
#[cfg(not(target_os = "windows"))]
pub(crate) fn owner_point(window: &Window, cx: &App, position: Point<Pixels>) -> Point<Pixels> {
    let id = Window::window_handle(window).window_id();
    cx.try_global::<Surfaces>().map_or(position, |state| position + owner_origin(state, id))
}
#[cfg(target_os = "windows")]
pub(crate) fn owner_point(window: &Window, cx: &App, position: Point<Pixels>) -> Point<Pixels> {
    let Some((_, parent)) = cx
        .try_global::<Surfaces>()
        .and_then(|state| state.owners.get(&Window::window_handle(window).window_id()))
    else {
        return position;
    };
    let Ok(raw) = HasWindowHandle::window_handle(window) else {
        return position;
    };
    oxyde_lib::platform::platform()
        .popup_point_to_owner(
            raw.as_raw(),
            *parent,
            (position.x.as_f32(), position.y.as_f32()),
        )
        .map(|(x, y)| point(px(x), px(y)))
        .unwrap_or(position)
}

pub(crate) fn close_for(window: &mut Window, cx: &mut App) {
    let id = Window::window_handle(window).window_id();
    let state = cx.default_global::<Surfaces>();
    state.pending.retain(|key, _| key.0 != id);
    let keys: Vec<_> = state
        .windows
        .iter()
        .filter(|(key, handle)| {
            key.0 == id
                || state
                    .logical_owners
                    .get(&handle.window_id())
                    .is_some_and(|parent| parent.window_id() == id)
        })
        .map(|(key, _)| key.clone())
        .collect();
    let handles: Vec<_> = keys
        .into_iter()
        .filter_map(|key| state.windows.remove(&key))
        .collect();
    for handle in handles {
        let _ = handle.update(cx, |_, window, cx| {
            close_for(window, cx);
            crate::ui::components::tooltip::hover::close_for(window, cx);
            window.remove_window();
        });
    }
}

#[cfg(target_os = "windows")]
fn family_active(spec: &Spec, _: &mut App) -> bool {
    oxyde_lib::platform::platform().popup_family_active(spec.raw_owner)
}
/// A grab ends with the compositor's dismissal, which covers leaving the application. Its menu
/// holds the keyboard, so the owner reads as inactive meanwhile.
#[cfg(not(target_os = "windows"))]
fn family_active(spec: &Spec, cx: &mut App) -> bool {
    spec.interactive || spec.parent.update(cx, |_, window, _| window.is_window_active()).unwrap_or(false)
}

fn restore(spec: &Spec, cx: &mut App) {
    if !spec.interactive {
        return;
    }
    if !family_active(spec, cx) {
        return;
    }
    let focus = spec.restore.clone();
    let _ = spec.restore_parent.update(cx, |_, window, cx| {
        // An action may already have focused a newly opened form or dialog.
        if window.focused(cx) == focus {
            if let Some(focus) = &focus {
                window.focus(focus, cx);
            }
        }
    });
}

pub(crate) fn close_all(cx: &mut App) {
    cx.default_global::<Surfaces>().pending.clear();
    let windows = std::mem::take(&mut cx.default_global::<Surfaces>().windows);
    for (_, handle) in windows {
        let _ = handle.update(cx, |_, window, cx| {
            crate::ui::components::tooltip::hover::close_for(window, cx);
            window.remove_window();
        });
    }
}

pub(crate) fn show<T: 'static>(
    id: impl Into<SharedString>,
    anchor: PopupAnchor,
    source: Entity<T>,
    parent: &mut Window,
    cx: &mut App,
    alive: impl Fn(&T) -> bool + 'static,
    factory: impl Fn(&mut T, &mut Window, &mut Context<T>) -> AnyElement + 'static,
    close: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
) {
    show_inner(id, anchor, source, parent, cx, alive, factory, close, true);
}

pub(crate) fn show_passive<T: 'static>(
    id: impl Into<SharedString>,
    anchor: PopupAnchor,
    source: Entity<T>,
    parent: &mut Window,
    cx: &mut App,
    alive: impl Fn(&T) -> bool + 'static,
    factory: impl Fn(&mut T, &mut Window, &mut Context<T>) -> AnyElement + 'static,
    close: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
) {
    show_inner(id, anchor, source, parent, cx, alive, factory, close, false);
}

fn show_inner<T: 'static>(
    id: impl Into<SharedString>,
    anchor: PopupAnchor,
    source: Entity<T>,
    parent: &mut Window,
    cx: &mut App,
    alive: impl Fn(&T) -> bool + 'static,
    factory: impl Fn(&mut T, &mut Window, &mut Context<T>) -> AnyElement + 'static,
    close: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    interactive: bool,
) {
    if !supported(parent, cx) {
        return;
    }
    let parent_handle = owner(parent, cx).unwrap_or(Window::window_handle(parent));
    let Ok(raw) = HasWindowHandle::window_handle(parent) else {
        return;
    };
    let raw_owner = cx
        .try_global::<Surfaces>()
        .and_then(|state| state.owners.get(&Window::window_handle(parent).window_id()))
        .map(|entry| entry.1)
        .unwrap_or(raw.as_raw());
    let key = (parent_handle.window_id(), id.into());
    let weak = source.downgrade();
    let make = weak.clone();
    let dismiss = weak.clone();
    #[cfg(target_os = "windows")]
    let raw_parent = anchor
        .parent
        .and_then(|handle| {
            handle
                .update(cx, |_, window, _| {
                    HasWindowHandle::window_handle(window)
                        .ok()
                        .map(|handle| handle.as_raw())
                })
                .ok()
                .flatten()
        })
        .unwrap_or(raw.as_raw());
    let restore_parent = anchor.parent.unwrap_or(Window::window_handle(parent));
    let restore_focus = if restore_parent == Window::window_handle(parent) {
        parent.focused(cx)
    } else {
        restore_parent
            .update(cx, |_, window, cx| window.focused(cx))
            .ok()
            .flatten()
    };
    let spec = Spec {
        key: key.clone(),
        parent: parent_handle,
        raw_owner,
        #[cfg(target_os = "windows")]
        raw_parent,
        restore_parent,
        anchor,
        rem: parent.rem_size(),
        restore: restore_focus,
        interactive,
        modifiers: parent.modifiers(),
        alive: Rc::new(move |cx| weak.upgrade().is_some_and(|view| alive(view.read(cx)))),
        factory: Rc::new(move |window, cx| {
            make.upgrade()
                .map(|view| view.update(cx, |view, cx| factory(view, window, cx)))
        }),
        close: Rc::new(move |cx| {
            let _ = parent_handle.update(cx, |_, window, cx| {
                if let Some(view) = dismiss.upgrade() {
                    view.update(cx, |view, cx| close(view, window, cx));
                }
            });
        }),
    };
    // The side of a surface is set at opening: replace one that must change it.
    #[cfg(not(target_os = "windows"))]
    if let Some(handle) = cx.default_global::<Surfaces>().windows.get(&key).copied() {
        let keeps = |anchor: &PopupAnchor| spec.anchor.parent.is_some() && !spec.anchor.beside && anchor.beside;
        if handle.read(cx).is_ok_and(|root| !keeps(&root.spec.anchor) && !super::wayland::same_placement(&root.spec.anchor, &spec.anchor, spec.rem * 2.0)) {
            cx.default_global::<Surfaces>().windows.remove(&key);
            let _ = handle.update(cx, |_, window, cx| {
                close_for(window, cx);
                crate::ui::components::tooltip::hover::close_for(window, cx);
                window.remove_window();
            });
        }
    }
    if let Some(handle) = cx.default_global::<Surfaces>().windows.get(&key).copied() {
        cx.defer(move |cx| {
            let _ = handle.update(cx, |root, window, cx| {
                let restore = root.spec.restore.clone();
                let mut spec = spec;
                if spec.anchor.parent.is_some() && !spec.anchor.beside && root.spec.anchor.beside {
                    spec.anchor = root.spec.anchor;
                }
                root.spec = spec;
                root.spec.restore = restore;
                #[cfg(not(target_os = "windows"))]
                cx.default_global::<Surfaces>().dismissals.insert(
                    Window::window_handle(window).window_id(),
                    (root.spec.alive.clone(), root.spec.close.clone()),
                );
                if window.rem_size() != root.spec.rem {
                    window.set_rem_size(root.spec.rem);
                }
                cx.notify();
                position(root, window, cx);
            });
        });
        return;
    }
    if cx
        .default_global::<Surfaces>()
        .pending
        .insert(key.clone(), spec)
        .is_some()
    {
        return;
    }
    if cx.default_global::<Surfaces>().cleanup.is_none() {
        let cleanup = cx.on_window_closed(|cx, id| {
            super::keyboard::remove(id, cx);
            // Closed by the compositor rather than by Oxyde: a click in another application.
            #[cfg(not(target_os = "windows"))]
            {
                let state = cx.default_global::<Surfaces>();
                state.origins.remove(&id);
                let dismissal = state.dismissals.remove(&id)
                    .filter(|_| state.windows.values().any(|handle| handle.window_id() == id));
                if let Some((alive, close)) = dismissal {
                    if alive(cx) {
                        cx.defer(move |cx| close(cx));
                    }
                }
            }
            let state = cx.default_global::<Surfaces>();
            state.failed.remove(&id);
            state.owners.remove(&id);
            state.logical_owners.remove(&id);
            state.raw_windows.remove(&id);
            state.rows.remove(&id);
            state.previous_rows.remove(&id);
            let keys: Vec<_> = state
                .windows
                .iter()
                .filter(|(key, handle)| key.0 == id || handle.window_id() == id)
                .map(|(key, _)| key.clone())
                .collect();
            let handles: Vec<_> = keys
                .into_iter()
                .filter_map(|key| state.windows.remove(&key))
                .collect();
            for handle in handles {
                if handle.window_id() != id {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                }
            }
        });
        cx.default_global::<Surfaces>().cleanup = Some(cleanup);
    }
    cx.defer(move |cx| {
        let Some(spec) = cx.default_global::<Surfaces>().pending.remove(&key) else {
            return;
        };
        if !(spec.alive)(cx) || cx.default_global::<Surfaces>().failed.contains_key(&key.0) {
            return;
        }

        #[cfg(not(target_os = "windows"))]
        if spec.interactive {
            close_unrelated(spec.restore_parent, cx);
        }
        let root_spec = spec.clone();
        let source_id = source.entity_id();
        #[cfg(target_os = "windows")]
        let Ok(origin) = spec
            .parent
            .update(cx, |_, window, _| window.bounds().origin)
        else {
            return;
        };
        #[cfg(target_os = "windows")]
        let (kind, show, bounds) = (WindowKind::PopUp, false, Bounds::new(origin, size(px(640.0), px(640.0))));
        #[cfg(not(target_os = "windows"))]
        let (kind, show, bounds) = {
            let anchor_bounds = spec.anchor.bounds - owner_origin(cx.default_global::<Surfaces>(), spec.restore_parent.window_id());
            let inset = spec.rem * 2.0;
            let options = super::wayland::options(spec.restore_parent, &spec.anchor, anchor_bounds, inset, spec.interactive);
            // As wide as its margins, to touch its anchor at any window edge: Mutter dismisses a
            // popup that does not touch its parent.
            let side = inset * 2.0 + px(1.0);
            (WindowKind::AnchoredPopup(options), true, Bounds::new(Point::default(), size(side, side)))
        };
        let result = cx.open_window(
            WindowOptions {
                app_id: Some(crate::ui::APP_ID.into()),
                kind,
                titlebar: None,
                show,
                focus: false,
                window_background: WindowBackgroundAppearance::Transparent,
                is_resizable: false,
                is_minimizable: false,
                is_movable: false,
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                window.set_rem_size(root_spec.rem);
                cx.default_global::<Surfaces>().logical_owners.insert(
                    Window::window_handle(window).window_id(),
                    root_spec.restore_parent,
                );
                cx.default_global::<Surfaces>().owners.insert(
                    Window::window_handle(window).window_id(),
                    (root_spec.parent, root_spec.raw_owner),
                );
                if let Ok(raw) = HasWindowHandle::window_handle(window) {
                    cx.default_global::<Surfaces>()
                        .raw_windows
                        .insert(Window::window_handle(window).window_id(), raw.as_raw());
                }
                let window_handle = Window::window_handle(window);
                #[cfg(not(target_os = "windows"))]
                {
                    window.set_input_region(Some(&[]));
                    let state = cx.default_global::<Surfaces>();
                    if let Some(offset) = window.popup_offset() {
                        state.origins.insert(window_handle.window_id(), offset);
                    }
                    state.dismissals.insert(window_handle.window_id(), (root_spec.alive.clone(), root_spec.close.clone()));
                }
                let root = cx.new(|cx: &mut Context<MenuWindow>| {
                    let observer = cx.observe(&source, |_, _, cx| cx.notify());
                    let watcher = cx.spawn(async move |this, cx| loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                        let alive = this
                            .update(cx, |root, cx| {
                                if !family_active(&root.spec, cx) {
                                    let close = root.spec.close.clone();
                                    cx.defer(move |cx| close(cx));
                                }
                                if !(root.spec.alive)(cx) {
                                    let handle = window_handle;
                                    cx.defer(move |cx| retire_if_closed(handle, cx));
                                    true
                                } else {
                                    true
                                }
                            })
                            .unwrap_or(false);
                        if !alive {
                            break;
                        }
                    });
                    MenuWindow {
                        spec: root_spec,
                        measured: None,
                        focus: cx.focus_handle(),
                        #[cfg(not(target_os = "windows"))]
                        ready: false,
                        #[cfg(not(target_os = "windows"))]
                        placed: std::cell::Cell::new(None),
                        _observer: observer,
                        _watcher: watcher,
                    }
                });
                let closing = root.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    let Some(root) = closing.upgrade() else {
                        return true;
                    };
                    let close_spec = root.read(cx).spec.clone();
                    crate::ui::components::tooltip::hover::close_for(window, cx);
                    if (close_spec.alive)(cx) {
                        (close_spec.close)(cx);
                    }
                    close_for(window, cx);
                    restore(&close_spec, cx);
                    true
                });
                root
            },
        );
        match result {
            Ok(handle) => {
                #[cfg(target_os = "windows")]
                let configured = handle.update(cx, |_, window, _| {
                    HasWindowHandle::window_handle(window)
                        .map_err(|error| error.to_string())
                        .and_then(|raw| {
                            oxyde_lib::platform::platform().configure_menu_window(
                                raw.as_raw(),
                                spec.raw_parent,
                                spec.raw_owner,
                                spec.interactive,
                            )
                        })
                });
                #[cfg(target_os = "windows")]
                if !matches!(configured, Ok(Ok(()))) {
                    log::warn!("Cannot configure native menu window");
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                    failed(spec.parent, source_id, cx);
                    return;
                }
                cx.default_global::<Surfaces>()
                    .owners
                    .insert(handle.window_id(), (spec.parent, spec.raw_owner));
                cx.default_global::<Surfaces>().windows.insert(key, handle);
                let _ = AnyWindowHandle::from(handle).update(cx, |_, window, cx| {
                    // Input dispatch may draw the window. Its root is installed
                    // only after open_window's constructor has returned.
                    window.dispatch_event(
                        PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                            modifiers: spec.modifiers,
                            ..Default::default()
                        }),
                        cx,
                    );
                });
                let _ = handle.update(cx, |root, window, cx| {
                    if root.spec.interactive {
                        window.focus(&root.focus, cx);
                    }
                    window.refresh();
                });
            }
            Err(error) => {
                log::warn!("Cannot open native menu: {error}");
                failed(spec.parent, source_id, cx);
            }
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn position(root: &MenuWindow, window: &Window, cx: &mut App) {
    let Some(measured) = root.measured else { return };
    let measured = size(measured.width.ceil(), measured.height.ceil());
    let state = cx.default_global::<Surfaces>();
    let bounds = root.spec.anchor.bounds - owner_origin(state, root.spec.restore_parent.window_id());
    let rect = super::wayland::rect(&root.spec.anchor, bounds, root.spec.rem * 2.0);
    if root.placed.get() != Some((rect, measured)) && window.reposition_popup(rect, measured) {
        root.placed.set(Some((rect, measured)));
    }
}

#[cfg(target_os = "windows")]
fn position(root: &MenuWindow, window: &Window, _: &mut App) {
    if let Ok(raw) = HasWindowHandle::window_handle(window) {
        let b = root.spec.anchor.bounds;
        let platform = oxyde_lib::platform::platform();
        let _ = platform.set_popup_anchor(
            raw.as_raw(),
            Some((
                b.left().as_f32(),
                b.top().as_f32(),
                b.size.width.as_f32(),
                b.size.height.as_f32(),
            )),
            root.spec.anchor.open_left,
            root.spec.anchor.beside,
            root.spec.anchor.above,
        );
        if let Some(measured) = root.measured {
            let _ = platform.place_hover_window(
                raw.as_raw(),
                Some((
                    measured.width.as_f32(),
                    measured.height.as_f32(),
                    (window.rem_size() * 2.0).as_f32(),
                )),
                window.scale_factor(),
            );
        }
    }
}

impl Render for MenuWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !(self.spec.alive)(cx) {
            let handle = Window::window_handle(window);
            cx.defer(move |cx| retire_if_closed(handle, cx));
            return div().into_any_element();
        }
        #[cfg(not(target_os = "windows"))]
        if !self.ready {
            let root = cx.entity();
            window.on_next_frame(move |_, cx| {
                root.update(cx, |root, cx| {
                    root.ready = true;
                    cx.notify();
                });
            });
            return div().into_any_element();
        }
        super::keyboard::begin(window, cx);
        cx.default_global::<Surfaces>()
            .rows
            .remove(&Window::window_handle(window).window_id());
        let content = (self.spec.factory)(window, cx).unwrap_or_else(|| div().into_any_element());
        let close = self.spec.close.clone();
        let outside = close.clone();
        let root = cx.entity();
        let parent = self.spec.parent;
        let focus = self.focus.clone();
        let wrapped = div()
            .id(self.spec.key.1.clone())
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .track_focus(&self.focus)
            .on_mouse_down_out(move |_, _, cx| outside(cx))
            .on_key_down(move |event, window, cx| {
                let modifiers = event.keystroke.modifiers;
                let plain = !(modifiers.control
                    || modifiers.alt
                    || modifiers.platform
                    || modifiers.function);
                if plain
                    && (event.keystroke.key == "escape"
                        || (event.keystroke.key == "left" && focus.is_focused(window)))
                {
                    close(cx);
                    cx.stop_propagation();
                } else if focus.is_focused(window) {
                    let key = if plain && event.keystroke.key == "tab" {
                        if modifiers.shift {
                            "up"
                        } else {
                            "down"
                        }
                    } else {
                        event.keystroke.key.as_str()
                    };
                    if plain && super::keyboard::key(key, window, cx) {
                        cx.stop_propagation();
                    } else if modifiers.control
                        || modifiers.alt
                        || matches!(key, "f2" | "delete" | "f5")
                    {
                        let keystroke = event.keystroke.clone();
                        let _ = parent
                            .update(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
                        cx.stop_propagation();
                    }
                }
            })
            .on_modifiers_changed(move |event, _, cx| {
                let event = event.clone();
                let _ = parent.update(cx, |_, window, cx| {
                    window.dispatch_event(PlatformInput::ModifiersChanged(event), cx)
                });
                cx.propagate();
            })
            .child(content)
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                super::keyboard::pointer(window, cx);
                                let mut event = event.clone();
                                event.position = super::owner_point(window, cx, event.position);
                                let _ = parent.update(cx, |view, window, cx| {
                                    if let Ok(view) = view.downcast::<crate::ui::app::OxydeApp>() {
                                        view.update(cx, |view, cx| view.move_popup_scroll(&event, window, cx));
                                    }
                                });
                                cx.propagate();
                            }
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                let mut event = event.clone();
                                event.position = super::owner_point(window, cx, event.position);
                                let _ = parent.update(cx, |view, _, cx| {
                                    if let Ok(view) = view.downcast::<crate::ui::app::OxydeApp>() {
                                        view.update(cx, |view, cx| view.release_popup_scroll(&event, cx));
                                    }
                                });
                                cx.propagate();
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
            .into_any_element();
        FramedContent {
            content: wrapped,
            root,
            inset: window.rem_size() * 2.0,
        }
        .into_any_element()
    }
}

fn retire_if_closed(handle: AnyWindowHandle, cx: &mut App) {
    let _ = handle.update(cx, |view, window, cx| {
        if let Ok(view) = view.downcast::<MenuWindow>() {
            let alive = view.read(cx).spec.alive.clone();
            if !alive(cx) {
                let spec = view.read(cx).spec.clone();
                crate::ui::components::tooltip::hover::close_for(window, cx);
                close_for(window, cx);
                restore(&spec, cx);
                window.remove_window();
            }
        }
    });
}
