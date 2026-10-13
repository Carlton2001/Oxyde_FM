//! Swap complete tooltip content, instead of showing a metadata-only frame
//! while GPUI resolves an already cached preview through its asset future.
use super::{scope::Scope, Sessions};
use crate::ui::components::tooltip::Tooltip;
use gpui::{AnyView, App, Window};
use std::time::{Duration, Instant};

const CHANGE_DELAY: Duration = Duration::from_millis(150);

fn can_publish(elapsed: Duration, preview_ready: bool, cached: bool) -> bool {
    preview_ready && (cached || elapsed >= CHANGE_DELAY)
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
    // An already resolved GPUI asset can be displayed without another frame.
    if ready(&view, cx) {
        super::publish_tooltip(view, window, cx);
        return;
    }
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
                let preview_ready = ready(&view, cx);
                let cached = view.clone().downcast::<Tooltip>().ok().is_some_and(|tooltip| tooltip.update(cx, |tooltip, cx| tooltip.cached_for_handover(cx)));
                if !can_publish(started.elapsed(), preview_ready, cached) {
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
