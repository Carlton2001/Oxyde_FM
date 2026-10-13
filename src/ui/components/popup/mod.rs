//! Owned transient surfaces. Business state and actions remain in the caller.
use gpui::*;
pub(crate) mod keyboard;

#[cfg(target_os = "windows")]
mod window;
#[cfg(target_os = "windows")]
pub(crate) use window::{show, close_all, supported, supported_source};
#[cfg(target_os = "windows")]
mod dropdown;
#[cfg(target_os = "windows")]
pub(crate) use dropdown::text_menu;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PopupAnchor { pub bounds: Bounds<Pixels>, pub open_left: bool, pub beside: bool, pub parent: Option<AnyWindowHandle> }
impl PopupAnchor {
    pub(crate) fn point(position: Point<Pixels>, open_left: bool) -> Self {
        Self { bounds: Bounds::new(position, size(px(0.0), px(0.0))), open_left, beside: false, parent: None }
    }
    pub(crate) fn element(bounds: Bounds<Pixels>) -> Self { Self { bounds, open_left: false, beside: false, parent: None } }
    pub(crate) fn beside(bounds: Bounds<Pixels>, open_left: bool) -> Self { Self { bounds, open_left, beside: true, parent: None } }
    pub(crate) fn parent(mut self, parent: Option<AnyWindowHandle>) -> Self { self.parent = parent; self }
}

pub(crate) fn owner_bounds(window: &Window, cx: &App, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    let origin = owner_point(window, cx, bounds.origin);
    let end = owner_point(window, cx, bounds.origin + point(bounds.size.width, bounds.size.height));
    Bounds::new(origin, size(end.x - origin.x, end.y - origin.y))
}
pub(crate) fn body_bounds(window: &Window, cx: &App) -> Bounds<Pixels> {
    let inset = window.rem_size() * 2.0;
    owner_bounds(window, cx, Bounds::new(point(inset, inset), size((window.viewport_size().width - inset * 2.0).max(px(0.0)), (window.viewport_size().height - inset * 2.0).max(px(0.0)))))
}

#[cfg(target_os = "windows")]
pub(crate) fn surface(parent: AnyWindowHandle, id: impl Into<SharedString>, cx: &App) -> Option<AnyWindowHandle> { window::surface(parent, id.into(), cx) }
#[cfg(target_os = "windows")]
pub(crate) fn row_bounds(parent: AnyWindowHandle, id: impl Into<SharedString>, index: usize, cx: &App) -> Option<Bounds<Pixels>> { window::row_bounds(parent, id.into(), index, cx) }
pub(crate) fn record_row(window: &Window, cx: &mut App, index: usize, bounds: Bounds<Pixels>) {
    #[cfg(target_os = "windows")]
    window::record_row(window, cx, index, bounds);
    #[cfg(not(target_os = "windows"))]
    let _ = (window, cx, index, bounds);
}

#[cfg(target_os = "windows")]
pub(crate) fn anchored_surface<T: 'static>(id: impl Into<SharedString>, source: Entity<T>, alive: impl Fn(&T) -> bool + 'static,
    factory: impl Fn(&mut T, &mut Window, &mut Context<T>) -> AnyElement + 'static,
    close: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static) -> impl IntoElement {
    let id = id.into();
    let alive = std::rc::Rc::new(alive); let factory = std::rc::Rc::new(factory); let close = std::rc::Rc::new(close);
    canvas(move |bounds, window, cx| {
        if !alive(source.read(cx)) { return; }
        let bounds = owner_bounds(window, cx, bounds);
        let alive = alive.clone(); let factory = factory.clone(); let close = close.clone();
        show(id.clone(), PopupAnchor::element(bounds), source.clone(), window, cx,
            move |view| alive(view), move |view, window, cx| factory(view, window, cx), move |view, window, cx| close(view, window, cx));
    }, |_, _, _, _| {}).absolute().size_full()
}

pub(crate) fn is_menu(window: &Window, cx: &App) -> bool {
    #[cfg(target_os = "windows")]
    { window::owner(window, cx).is_some() }
    #[cfg(not(target_os = "windows"))]
    { let _ = (window, cx); false }
}

pub(crate) fn owner_point(window: &Window, cx: &App, position: Point<Pixels>) -> Point<Pixels> {
    #[cfg(target_os = "windows")]
    { return window::owner_point(window, cx, position); }
    #[cfg(not(target_os = "windows"))]
    { let _ = (window, cx); position }
}

pub(crate) fn with_owner(window: &mut Window, cx: &mut App, callback: impl FnOnce(&mut Window, &mut App)) {
    #[cfg(target_os = "windows")]
    if let Some(parent) = window::owner(window, cx) {
        let _ = parent.update(cx, |_, window, cx| callback(window, cx));
        return;
    }
    callback(window, cx);
}

pub(crate) fn available_size(window: &Window, cx: &App) -> Size<Pixels> {
    if is_menu(window, cx) {
        let area = raw_window_handle::HasWindowHandle::window_handle(window).ok()
            .and_then(|handle| oxyde_lib::platform::platform().popup_work_area_size(handle.as_raw()).ok())
            .map(|(width, height)| size(px(width), px(height))).unwrap_or(window.viewport_size());
        let margin = window.rem_size() * 4.0;
        size((area.width - margin).max(px(1.0)), (area.height - margin).max(px(1.0)))
    } else { window.viewport_size() }
}
