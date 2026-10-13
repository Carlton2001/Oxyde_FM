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
    measured: Size<Pixels>,
    scale: f32,
}

fn candidate(view: &AnyView, cx: &App) -> Option<Entity<Tooltip>> {
    let tooltip = view.clone().downcast::<Tooltip>().ok()?;
    let preview = tooltip.read(cx).preview_view.as_ref()?.read(cx);
    (preview.original_image.is_some() || preview.media_asset.is_some()).then_some(tooltip)
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
    if state.windows.contains_key(&id) || !state.pending.insert(id) {
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
                            measured: size(px(1.0), px(1.0)),
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
                let task = cx.spawn(async move |cx| loop {
                    let alive = cx.update(|cx| {
                        parent_handle
                            .update(cx, |_, parent, cx| {
                                let session = cx.default_global::<Sessions>().windows.get(&id).map(
                                    |session| (session.scope.clone(), session.tooltip.clone()),
                                );
                                let view = session.and_then(|(scope, view)| {
                                    scope.filter(|scope| scope.valid(parent, cx)).and(view)
                                });
                                let view = view.as_ref().and_then(|view| candidate(view, cx));
                                let rem = parent.rem_size();
                                handle
                                    .update(cx, |root, window, cx| {
                                        let changed = root.view.as_ref().map(Entity::entity_id)
                                            != view.as_ref().map(Entity::entity_id);
                                        if changed {
                                            if let Some(old) = root.view.take() {
                                                old.update(cx, |tooltip, _| {
                                                    tooltip.native_presentation = false
                                                });
                                            }
                                            root.view = view;
                                            if let Some(view) = &root.view {
                                                view.update(cx, |tooltip, _| {
                                                    tooltip.native_presentation = true
                                                });
                                            }
                                            window.refresh();
                                        }
                                        if window.rem_size() != rem {
                                            window.set_rem_size(rem);
                                            window.refresh();
                                        }
                                        root.scale = window.scale_factor();
                                        if let Ok(raw) = HasWindowHandle::window_handle(window) {
                                            let size = root.view.as_ref().map(|_| {
                                                (
                                                    root.measured.width.as_f32(),
                                                    root.measured.height.as_f32(),
                                                )
                                            });
                                            if let Err(error) = oxyde_lib::platform::platform()
                                                .place_hover_window(raw.as_raw(), size, root.scale)
                                            {
                                                log::warn!("Cannot place hover window: {error}");
                                            }
                                        }
                                    })
                                    .is_ok()
                            })
                            .unwrap_or(false)
                    });
                    if !alive {
                        break;
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                });
                cx.default_global::<NativeWindows>()
                    .windows
                    .insert(id, (handle, task));
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
        if self.owner.read(cx).measured != *measured {
            let owner = self.owner.clone();
            let measured = *measured;
            cx.defer(move |cx| owner.update(cx, |root, _| root.measured = measured));
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
