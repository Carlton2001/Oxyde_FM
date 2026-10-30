//! A window-local tooltip session shared inside each hover region.
mod scope;
mod handover;
pub(super) mod native_window;
#[cfg(not(target_os = "windows"))]
mod anchored;

pub(crate) fn close_native_windows(cx: &mut App) {
    native_window::close_windows(cx);
}
use crate::ui::app::OxydeApp;
use gpui::*;
pub(crate) use scope::HoverRegion;
use scope::Scope;
use std::{collections::HashMap, rc::Rc, time::Duration};

const SHOW_DELAY: Duration = Duration::from_millis(750);
const BRIDGE_DELAY: Duration = Duration::from_millis(300);
/// A compositor shows every frame of a window, so hiding at once would blink each time the
/// pointer crosses the gap between two items. A Windows surface hides without a frame.
#[cfg(not(target_os = "windows"))]
const GAP_DELAY: Duration = Duration::from_millis(80);
type Builder = Rc<dyn Fn(&mut Window, &mut App) -> AnyView>;

#[derive(Default)]
struct Timing {
    hovered: Option<String>,
    // A displayed session stays warm through a gap even while its view is hidden.
    warm: bool,
    revision: u64,
}

impl Timing {
    fn reset(&mut self) {
        self.revision += 1;
        self.hovered = None;
        self.warm = false;
    }
    fn enter(&mut self, key: String) -> bool {
        self.revision += 1;
        self.hovered = Some(key);
        self.warm
    }
    fn leave(&mut self, key: &str) -> bool {
        if self.hovered.as_deref() != Some(key) {
            return false;
        }
        self.revision += 1;
        self.hovered = None;
        true
    }
    fn show(&mut self, revision: u64, key: &str) -> bool {
        if self.revision != revision || self.hovered.as_deref() != Some(key) {
            return false;
        }
        self.warm = true;
        true
    }
    fn in_gap(&self, revision: u64) -> bool {
        self.revision == revision && self.hovered.is_none()
    }
    fn expire(&mut self, revision: u64) -> bool {
        if !self.in_gap(revision) {
            return false;
        }
        self.warm = false;
        true
    }
}

#[derive(Default)]
struct Session {
    scope: Option<Scope>,
    timing: Timing,
    tooltip: Option<AnyView>,
    task: Option<Task<()>>,
}
impl Session {
    fn clear(&mut self) {
        self.timing.reset();
        self.tooltip = None;
        self.scope = None;
        self.task = None;
    }
}

#[derive(Default)]
struct Sessions {
    windows: HashMap<WindowId, Session>,
    cleanup: Option<Subscription>,
}
impl Global for Sessions {}

fn initialize(cx: &mut App) {
    if cx.default_global::<Sessions>().cleanup.is_none() {
        let subscription = cx.on_window_closed(|cx, id| {
            cx.default_global::<Sessions>().windows.remove(&id);
        });
        cx.default_global::<Sessions>().cleanup = Some(subscription);
    }
}

pub(crate) fn close_for(window: &mut Window, cx: &mut App) {
    dismiss(window, cx);
    native_window::close_for(window.window_handle().window_id(), cx);
}

pub(crate) fn dismiss(window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    native_window::hide(id, cx);
    if let Some(session) = cx.default_global::<Sessions>().windows.get_mut(&id) {
        session.clear();
    }
    window.refresh();
}

fn clear(window: &mut Window, cx: &mut App) {
    dismiss(window, cx);
}

fn hide_gap(revision: u64, window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    let hidden = cx
        .default_global::<Sessions>()
        .windows
        .get_mut(&id)
        .filter(|session| session.timing.in_gap(revision))
        .and_then(|session| session.tooltip.take())
        .is_some();
    if hidden {
        native_window::blank(id, cx);
        window.refresh();
    }
}

fn expire_bridge(revision: u64, window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    let expired = cx
        .default_global::<Sessions>()
        .windows
        .get_mut(&id)
        .is_some_and(|session| session.timing.expire(revision));
    if expired {
        // The native surface outlives the session: hide it and release its
        // retained frames too, even if no further pointer event arrives.
        clear(window, cx);
    }
}

fn publish_tooltip(view: AnyView, window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    cx.default_global::<Sessions>().windows.get_mut(&id).unwrap().tooltip = Some(view);
    native_window::sync(window, cx);
    window.refresh();
}

fn hover(
    scope: Scope,
    key: String,
    builder: Builder,
    hovered: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let id = window.window_handle().window_id();
    if hovered && !scope.valid(window, cx) {
        clear(window, cx);
        return;
    }
    let session = cx
        .default_global::<Sessions>()
        .windows
        .entry(id)
        .or_default();
    if hovered {
        if session.scope.as_ref().map(|s| &s.tag) != Some(&scope.tag) {
            session.clear();
        }
        session.scope = Some(scope.clone());
        session.task = None;
        let immediate = session.timing.enter(key.clone());
        let revision = session.timing.revision;
        if immediate {
            let tooltip = builder(window, cx);
            handover::replace(scope, key, revision, tooltip, window, cx);
        } else {
            // Prepare during the initial hover delay, without publishing a
            // metadata-only bubble. The cancellable task owns the pending view.
            let tooltip = builder(window, cx);
            if let Ok(view) = tooltip.clone().downcast::<super::Tooltip>() {
                view.update(cx, |tooltip, cx| {
                    // Image/PDF assets are lazy: construction alone does not
                    // start their worker load. Fetch without waiting for it.
                    tooltip.ready_for_initial_display(cx);
                });
            }
            let task = window.spawn(cx, async move |cx| {
                cx.background_executor().timer(SHOW_DELAY).await;
                loop {
                    let done = cx.update(|window, cx| {
                        let current = cx
                            .default_global::<Sessions>()
                            .windows
                            .get(&id)
                            .is_some_and(|session| {
                                session.timing.revision == revision
                                    && session.timing.hovered.as_deref() == Some(&key)
                            });
                        if !current {
                            return true;
                        }
                        if !scope.valid(window, cx) {
                            clear(window, cx);
                            return true;
                        }
                        if let Ok(view) = tooltip.clone().downcast::<super::Tooltip>() {
                            if !view.update(cx, |tooltip, cx| tooltip.ready_for_initial_display(cx)) {
                                return false;
                            }
                        }
                        let session = cx
                            .default_global::<Sessions>()
                            .windows
                            .get_mut(&id)
                            .unwrap();
                        if !session.timing.show(revision, &key) {
                            return true;
                        }
                        publish_tooltip(tooltip.clone(), window, cx);
                        true
                    }).unwrap_or(true);
                    if done {
                        break;
                    }
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                }
            });
            cx.default_global::<Sessions>()
                .windows
                .get_mut(&id)
                .unwrap()
                .task = Some(task);
        }
    } else {
        if session.scope.as_ref().map(|s| &s.tag) != Some(&scope.tag) || !session.timing.leave(&key)
        {
            return;
        }
        session.task = None;
        if !session.timing.warm {
            session.clear();
            return;
        }
        let revision = session.timing.revision;
        // Let all hover listeners handle this movement first: a direct switch
        // to another item must not briefly hide the shared surface.
        #[cfg(target_os = "windows")]
        window.defer(cx, move |window, cx| hide_gap(revision, window, cx));
        let task = window.spawn(cx, async move |cx| {
            #[cfg(not(target_os = "windows"))]
            {
                cx.background_executor().timer(GAP_DELAY).await;
                let _ = cx.update(|window, cx| hide_gap(revision, window, cx));
                cx.background_executor().timer(BRIDGE_DELAY - GAP_DELAY).await;
            }
            #[cfg(target_os = "windows")]
            cx.background_executor().timer(BRIDGE_DELAY).await;
            let _ = cx.update(|window, cx| {
                expire_bridge(revision, window, cx);
            });
        });
        cx.default_global::<Sessions>()
            .windows
            .get_mut(&id)
            .unwrap()
            .task = Some(task);
        window.refresh();
    }
}

pub(crate) trait SharedTooltipExt {
    fn shared_tooltip(
        self,
        owner: Entity<OxydeApp>,
        region: HoverRegion,
        key: String,
        cx: &mut App,
        builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> Self;
    fn file_tooltip(
        self,
        owner: Entity<OxydeApp>,
        pane: String,
        key: String,
        cx: &mut App,
        builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> Self;
}

impl SharedTooltipExt for Stateful<Div> {
    fn file_tooltip(
        self,
        owner: Entity<OxydeApp>,
        pane: String,
        key: String,
        cx: &mut App,
        builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> Self {
        self.shared_tooltip(owner, HoverRegion::Files(pane), key, cx, builder)
    }

    fn shared_tooltip(
        self,
        owner: Entity<OxydeApp>,
        region: HoverRegion,
        key: String,
        cx: &mut App,
        builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> Self {
        initialize(cx);
        let owner = owner.downgrade();
        let builder: Builder = Rc::new(builder);
        self.on_hover(move |hovered, window, cx| {
            let Some(owner) = owner.upgrade() else {
                clear(window, cx);
                return;
            };
            let Some(scope) = Scope::new(&owner, &region, cx) else {
                return;
            };
            hover(
                scope.clone(),
                key.clone(),
                builder.clone(),
                *hovered,
                window,
                cx,
            )
        })
        .on_mouse_down(MouseButton::Left, |_, window, cx| clear(window, cx))
        .on_mouse_down(MouseButton::Right, |_, window, cx| clear(window, cx))
    }
}

/// Gaps hide the tooltip immediately while keeping the session warm briefly.
/// Leaving the list, navigation, scrolling or a context menu ends the session.
pub(crate) fn container(
    owner: Entity<OxydeApp>,
    pane: String,
    top: Pixels,
    bottom: Pixels,
    cx: &mut App,
) -> impl IntoElement {
    region_container(owner, HoverRegion::Files(pane), top, bottom, cx)
}

pub(crate) fn region_container(
    owner: Entity<OxydeApp>,
    region: HoverRegion,
    top: Pixels,
    bottom: Pixels,
    cx: &mut App,
) -> impl IntoElement {
    initialize(cx);
    let owner = owner.downgrade();
    let follow_region = region.clone();
    canvas(
        move |bounds, window, cx| {
            let Some(owner) = owner.upgrade() else {
                clear(window, cx);
                return;
            };
            let Some(scope) = Scope::new(&owner, &region, cx) else {
                return;
            };
            let id = window.window_handle().window_id();
            let session = cx
                .default_global::<Sessions>()
                .windows
                .entry(id)
                .or_default();
            let Some(active) = session.scope.clone() else {
                return;
            };
            // Another region's container also releases a session whose own
            // container disappeared, for example when the tree is collapsed.
            if !active.valid(window, cx) {
                clear(window, cx);
                return;
            }
            if !active.same_region(&scope) {
                return;
            }
            if active.tag != scope.tag || !bounds.contains(&window.mouse_position()) {
                clear(window, cx);
                return;
            }
            let session = cx.default_global::<Sessions>().windows.get(&id).unwrap();
            if let Some(view) = session.tooltip.clone() {
                if native_window::present(&view, window, cx) {
                    return;
                }
                let tag = scope.tag.clone();
                window.set_tooltip(AnyTooltip {
                    view,
                    mouse_position: window.mouse_position(),
                    check_visible_and_update: Rc::new(move |_, window, cx| {
                        let valid = bounds.contains(&window.mouse_position())
                            && active.valid(window, cx)
                            && cx
                                .default_global::<Sessions>()
                                .windows
                                .get(&id)
                                .is_some_and(|session| {
                                    session.scope.as_ref().is_some_and(|scope| scope.tag == tag)
                                        && session.tooltip.is_some()
                                });
                        let still_current = cx
                            .default_global::<Sessions>()
                            .windows
                            .get(&id)
                            .is_some_and(|session| {
                                session.scope.as_ref().is_some_and(|scope| scope.tag == tag)
                            });
                        if !valid && still_current {
                            clear(window, cx);
                        }
                        valid
                    }),
                });
            }
        },
        move |bounds, _, window, _| {
            let region = follow_region.clone();
            window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    native_window::follow_pointer(&region, bounds, window, cx);
                }
            });
        },
    )
    .absolute()
    .left_0()
    .right_0()
    .top(top)
    .bottom(bottom)
}

#[cfg(test)]
#[path = "tests/hover_tests.rs"]
mod tests;
