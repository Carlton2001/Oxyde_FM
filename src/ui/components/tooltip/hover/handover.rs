//! Swap complete tooltip content, instead of showing a metadata-only frame
//! while GPUI resolves an already cached preview through its asset future.
use super::{scope::Scope, Sessions};
use crate::ui::components::tooltip::Tooltip;
use gpui::{AnyView, App, Window};
use std::time::{Duration, Instant};

const CHANGE_DELAY: Duration = Duration::from_millis(150);

fn can_publish(elapsed: Duration, preview_ready: bool) -> bool {
    elapsed >= CHANGE_DELAY && preview_ready
}

fn ready(view: &AnyView, cx: &mut App) -> bool {
    match view.clone().downcast::<Tooltip>() {
        Ok(tooltip) => tooltip.update(cx, |tooltip, cx| tooltip.ready_for_handover(cx)),
        Err(_) => true,
    }
}

pub(super) fn replace(
    scope: Scope,
    key: String,
    revision: u64,
    view: AnyView,
    window: &mut Window,
    cx: &mut App,
) {
    let id = window.window_handle().window_id();
    // Start loading now, including cache lookup, while keeping the old bubble
    // intact. Even a cached preview waits for a stable hover before replacement.
    let _ = ready(&view, cx);
    let started = Instant::now();
    // The pending view belongs to the cancellable task, not to the painted
    // tooltip. Leaving or entering a different item releases it immediately.
    let task = window.spawn(cx, async move |cx| loop {
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        let done = cx
            .update(|window, cx| {
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
                    super::clear(window, cx);
                    return true;
                }
                if !can_publish(started.elapsed(), ready(&view, cx)) {
                    return false;
                }
                super::publish_tooltip(view.clone(), window, cx);
                true
            })
            .unwrap_or(true);
        if done {
            break;
        }
    });
    cx.default_global::<Sessions>()
        .windows
        .get_mut(&id)
        .unwrap()
        .task = Some(task);
}

#[cfg(test)]
#[path = "../tests/handover_tests.rs"]
mod tests;
