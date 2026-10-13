//! A window-local tooltip session shared inside each hover region.
mod scope;
mod handover;
#[cfg(target_os = "windows")]
mod native_window;

pub(crate) fn close_native_windows(cx: &mut App) {
    #[cfg(target_os = "windows")]
    native_window::close_windows(cx);
    #[cfg(not(target_os = "windows"))]
    let _ = cx;
}
use crate::ui::app::OxydeApp;
use gpui::*;
pub(crate) use scope::HoverRegion;
use scope::Scope;
use std::{collections::HashMap, rc::Rc, time::Duration};

const SHOW_DELAY: Duration = Duration::from_millis(750);
const BRIDGE_DELAY: Duration = Duration::from_millis(300);
type Builder = Rc<dyn Fn(&mut Window, &mut App) -> AnyView>;

#[derive(Default)]
struct Timing {
    hovered: Option<String>,
    visible: bool,
    revision: u64,
}

impl Timing {
    fn reset(&mut self) {
        self.revision += 1;
        self.hovered = None;
        self.visible = false;
    }
    fn enter(&mut self, key: String) -> bool {
        self.revision += 1;
        self.hovered = Some(key);
        self.visible
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
        self.visible = true;
        true
    }
    fn expire(&mut self, revision: u64) -> bool {
        if self.revision != revision || self.hovered.is_some() {
            return false;
        }
        self.visible = false;
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

pub(crate) fn dismiss(window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    #[cfg(target_os = "windows")]
    native_window::hide(id, cx);
    if let Some(session) = cx.default_global::<Sessions>().windows.get_mut(&id) {
        session.clear();
    }
    window.refresh();
}

fn clear(window: &mut Window, cx: &mut App) {
    dismiss(window, cx);
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
            let task = window.spawn(cx, async move |cx| {
                cx.background_executor().timer(SHOW_DELAY).await;
                let _ = cx.update(|window, cx| {
                    let current = cx
                        .default_global::<Sessions>()
                        .windows
                        .get(&id)
                        .is_some_and(|session| {
                            session.timing.revision == revision
                                && session.timing.hovered.as_deref() == Some(&key)
                        });
                    if !current {
                        return;
                    }
                    if !scope.valid(window, cx) {
                        clear(window, cx);
                        return;
                    }
                    let session = cx
                        .default_global::<Sessions>()
                        .windows
                        .get_mut(&id)
                        .unwrap();
                    if !session.timing.show(revision, &key) {
                        return;
                    }
                    let tooltip = builder(window, cx);
                    let session = cx
                        .default_global::<Sessions>()
                        .windows
                        .get_mut(&id)
                        .unwrap();
                    session.tooltip = Some(tooltip);
                    window.refresh();
                });
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
        if !session.timing.visible {
            session.clear();
            return;
        }
        let revision = session.timing.revision;
        let task = window.spawn(cx, async move |cx| {
            cx.background_executor().timer(BRIDGE_DELAY).await;
            let _ = cx.update(|window, cx| {
                let session = cx
                    .default_global::<Sessions>()
                    .windows
                    .get_mut(&id)
                    .unwrap();
                if session.timing.expire(revision) {
                    session.clear();
                    window.refresh();
                }
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

/// The container keeps the previous tooltip through short gaps, and releases it
/// immediately outside the list, on navigation, scrolling or a context menu.
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
                #[cfg(target_os = "windows")]
                if matches!(scope.region, HoverRegion::Files(_)) && native_window::present(&view, window, cx) {
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
        |_, _, _, _| {},
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
