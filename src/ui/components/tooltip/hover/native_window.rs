//! Windows prototype: reuse one passive native surface for image/PDF tooltips.
use super::Sessions;
use crate::ui::components::tooltip::Tooltip;
use gpui::*;
use raw_window_handle::HasWindowHandle;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

#[derive(Default)]
struct NativeWindows {
    windows: HashMap<WindowId, (WindowHandle<NativeTooltip>, Task<()>)>,
    pending: HashSet<WindowId>,
    failed: HashSet<WindowId>,
    cleanup: Option<Subscription>,
}
impl Global for NativeWindows {}

pub(super) struct NativeTooltip {
    view: Option<Entity<Tooltip>>,
    measured: Option<Size<Pixels>>,
    scale: f32,
}

fn visible_size(measured: Option<Size<Pixels>>, rem: Pixels) -> Size<Pixels> {
    measured
        .filter(|size| {
            size.width.as_f32().is_finite()
                && size.height.as_f32().is_finite()
                && size.width > px(0.0)
                && size.height > px(0.0)
        })
        .unwrap_or_else(|| size(rem * 26.5, rem * 8.0))
}

pub(super) fn close_windows(cx: &mut App) {
    let entries = std::mem::take(&mut cx.default_global::<NativeWindows>().windows);
    for (_, (handle, task)) in entries {
        drop(task);
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}

fn candidate(view: &AnyView, cx: &App) -> Option<Entity<Tooltip>> {
    let tooltip = view.clone().downcast::<Tooltip>().ok()?;
    let preview = tooltip.read(cx).preview_view.as_ref()?.read(cx);
    (preview.original_image.is_some() || preview.media_asset.is_some()).then_some(tooltip)
}

pub(super) fn immediate_handover(view: &AnyView, window: &Window, cx: &mut App) -> bool {
    candidate(view, cx).is_some()
        && !cx
            .default_global::<NativeWindows>()
            .failed
            .contains(&window.window_handle().window_id())
}

fn place(root: &NativeTooltip, window: &Window) {
    if let Ok(raw) = HasWindowHandle::window_handle(window) {
        let size = root.view.as_ref().map(|_| {
            let measured = visible_size(root.measured, window.rem_size());
            (measured.width.as_f32(), measured.height.as_f32())
        });
        if let Err(error) = oxyde_lib::platform::platform().place_hover_window(
            raw.as_raw(),
            size,
            window.scale_factor(),
        ) {
            log::warn!("Cannot place hover window: {error}");
        }
    }
}

/// Called by pointer events and content publication, never by a movement timer.
pub(super) fn sync(parent: &mut Window, cx: &mut App) {
    let id = Window::window_handle(parent).window_id();
    let handle = cx
        .default_global::<NativeWindows>()
        .windows
        .get(&id)
        .map(|entry| entry.0);
    let Some(handle) = handle else {
        return;
    };
    let session = cx
        .default_global::<Sessions>()
        .windows
        .get(&id)
        .map(|session| (session.scope.clone(), session.tooltip.clone()));
    let view =
        session.and_then(|(scope, view)| scope.filter(|scope| scope.valid(parent, cx)).and(view));
    let view = view.as_ref().and_then(|view| candidate(view, cx));
    let rem = parent.rem_size();
    let _ = handle.update(cx, |root, window, cx| {
        if root.view.as_ref().map(Entity::entity_id) != view.as_ref().map(Entity::entity_id) {
            if let Some(old) = root.view.take() {
                old.update(cx, |tooltip, _| tooltip.native_presentation = false);
            }
            root.view = view;
            if let Some(view) = &root.view {
                view.update(cx, |tooltip, cx| {
                    tooltip.native_presentation = true;
                    cx.notify();
                });
            }
            cx.notify();
            window.refresh();
        }
        if window.rem_size() != rem {
            window.set_rem_size(rem);
            window.refresh();
        }
        place(root, window);
    });
}

pub(super) fn follow_pointer(
    region: &super::HoverRegion,
    bounds: Bounds<Pixels>,
    parent: &mut Window,
    cx: &mut App,
) {
    let id = Window::window_handle(parent).window_id();
    let owns_session = cx
        .default_global::<Sessions>()
        .windows
        .get(&id)
        .is_some_and(|session| {
            session
                .scope
                .as_ref()
                .is_some_and(|scope| &scope.region == region)
        });
    if !owns_session {
        return;
    }
    if bounds.contains(&parent.mouse_position()) {
        sync(parent, cx);
    } else {
        super::clear(parent, cx);
    }
}

pub(super) fn hide(id: WindowId, cx: &mut App) {
    let handle = cx
        .default_global::<NativeWindows>()
        .windows
        .get(&id)
        .map(|entry| entry.0);
    if let Some(handle) = handle {
        let _ = handle.update(cx, |root, window, cx| {
            if let Some(view) = root.view.take() {
                view.update(cx, |tooltip, _| tooltip.native_presentation = false);
            }
            if let Ok(handle) = window.window_handle() {
                let _ = oxyde_lib::platform::platform().place_hover_window(
                    handle.as_raw(),
                    None,
                    root.scale,
                );
            }
        });
    }
}

pub(super) fn present(view: &AnyView, parent: &mut Window, cx: &mut App) -> bool {
    if candidate(view, cx).is_none() {
        return false;
    }
    let id = Window::window_handle(parent).window_id();
    let state = cx.default_global::<NativeWindows>();
    if state.failed.contains(&id) {
        return false;
    }
    if state.windows.contains_key(&id) {
        let parent = Window::window_handle(parent);
        cx.defer(move |cx| {
            let _ = parent.update(cx, |_, window, cx| sync(window, cx));
        });
        return true;
    }
    if !state.pending.insert(id) {
        return true;
    }
    if state.cleanup.is_none() {
        let cleanup = cx.on_window_closed(|cx, id| {
            let entry = cx.default_global::<NativeWindows>().windows.remove(&id);
            cx.default_global::<NativeWindows>().pending.remove(&id);
            cx.default_global::<NativeWindows>().failed.remove(&id);
            if let Some((handle, _)) = entry {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        });
        cx.default_global::<NativeWindows>().cleanup = Some(cleanup);
    }
    let owner = parent.window_handle().ok().map(|handle| handle.as_raw());
    let parent_handle = Window::window_handle(parent);
    let rem = parent.rem_size();
    let scale = parent.scale_factor();
    cx.defer(move |cx| {
        let result = (|| -> Result<WindowHandle<NativeTooltip>, String> {
            parent_handle
                .update(cx, |_, _, _| ())
                .map_err(|error| error.to_string())?;
            let owner = owner.ok_or_else(|| "Missing hover owner".to_string())?;
            let handle = cx
                .open_window(
                    WindowOptions {
                        kind: WindowKind::PopUp,
                        titlebar: None,
                        focus: false,
                        show: false,
                        is_resizable: false,
                        is_minimizable: false,
                        is_movable: false,
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(0.0), px(0.0)),
                            size(px(424.0), px(544.0)),
                        ))),
                        ..Default::default()
                    },
                    |window, cx| {
                        window.set_rem_size(rem);
                        cx.new(|_| NativeTooltip {
                            view: None,
                            measured: None,
                            scale,
                        })
                    },
                )
                .map_err(|error| error.to_string())?;
            let configured = handle
                .update(cx, |_, window, _| {
                    let raw = HasWindowHandle::window_handle(window)
                        .map_err(|error| error.to_string())?;
                    oxyde_lib::platform::platform().configure_hover_window(raw.as_raw(), owner)
                })
                .map_err(|error| error.to_string())?;
            if let Err(error) = configured {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                return Err(error);
            }
            Ok(handle)
        })();
        cx.default_global::<NativeWindows>().pending.remove(&id);
        match result {
            Ok(handle) => {
                // Only invalidate a hidden/left/deactivated session here. Movement,
                // content and measured-size changes are event-driven.
                let task = cx.spawn(async move |cx| loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let alive = cx.update(|cx| {
                        parent_handle
                            .update(cx, |_, parent, cx| {
                                let scope = cx
                                    .default_global::<Sessions>()
                                    .windows
                                    .get(&id)
                                    .and_then(|session| session.scope.clone());
                                if !scope.is_some_and(|scope| scope.valid(parent, cx)) {
                                    hide(id, cx);
                                }
                            })
                            .is_ok()
                    });
                    if !alive {
                        break;
                    }
                });
                cx.default_global::<NativeWindows>()
                    .windows
                    .insert(id, (handle, task));
                let _ = parent_handle.update(cx, |_, parent, cx| sync(parent, cx));
            }
            Err(error) => {
                log::warn!("Native hover preview unavailable: {error}");
                cx.default_global::<NativeWindows>().failed.insert(id);
                let _ = parent_handle.update(cx, |_, window, _| window.refresh());
            }
        }
    });
    true
}

impl Render for NativeTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.view {
            Some(view) => view.clone().into_any_element(),
            None => div().into_any_element(),
        };
        MeasuredContent {
            content,
            owner: cx.entity(),
        }
    }
}

struct MeasuredContent {
    content: AnyElement,
    owner: Entity<NativeTooltip>,
}
impl IntoElement for MeasuredContent {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for MeasuredContent {
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
        let mut style = Style::default();
        style.size = size(measured.width.into(), measured.height.into());
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
        self.content
            .prepaint_at(point(px(0.0), px(0.0)), window, cx);
        if measured.width > px(0.0)
            && measured.height > px(0.0)
            && self.owner.read(cx).measured != Some(*measured)
        {
            let owner = self.owner.clone();
            let measured = *measured;
            let handle = Window::window_handle(window);
            cx.defer(move |cx| {
                owner.update(cx, |root, _| root.measured = Some(measured));
                let _ = handle.update(cx, |_, window, cx| {
                    let root = owner.read(cx);
                    place(root, window);
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

#[cfg(test)]
#[path = "../tests/native_dimensions.rs"]
mod tests;
