//! Glide the breadcrumb toward its end when entering a subfolder, and along wheel scrolls.
use gpui::*;
use std::{cell::Cell, time::Instant};

/// Entering a subfolder reveals a new segment: slow enough to follow.
const ENTER_DURATION: f32 = 0.22;
/// Matches the tab strip's wheel scrolling.
const WHEEL_DURATION: f32 = 0.18;

type Glide = Cell<Option<(Instant, f32, Option<f32>)>>;

pub(super) fn start(glide: &Glide, previous: &str, current: &str, scroll: &ScrollHandle) {
    let entered = entered_child(previous, current);
    glide.set(entered.then(|| (Instant::now(), scroll.offset().x.as_f32(), None)));
}

/// Retargets the glide by `delta` from the pending wheel target, so fast wheel turns accumulate.
pub(super) fn scroll_by(glide: &Glide, scroll: &ScrollHandle, delta: Pixels) {
    let current = scroll.offset().x.as_f32();
    let base = match glide.get() {
        Some((_, _, Some(target))) => target,
        _ => current,
    };
    let target = (base + delta.as_f32()).clamp(-scroll.max_offset().x.as_f32(), 0.0);
    glide.set(Some((Instant::now(), current, Some(target))));
}

/// Moves the scroll offset one frame forward; returns whether a glide is running.
pub(super) fn advance(glide: &Glide, scroll: &ScrollHandle, window: &Window) -> bool {
    let Some((started, from, target)) = glide.get() else {
        return false;
    };
    let max = scroll.max_offset().x.as_f32();
    let (target, duration) = match target {
        Some(target) => (target.clamp(-max, 0.0), WHEEL_DURATION),
        None => (-max, ENTER_DURATION),
    };
    let t = (started.elapsed().as_secs_f32() / duration).min(1.0);
    let eased = 1.0 - (1.0 - t).powi(3);
    scroll.set_offset(point(px(from + (target - from) * eased), px(0.0)));
    if t < 1.0 {
        window.request_animation_frame();
    } else {
        glide.set(None);
    }
    true
}

/// Whether `current` is exactly one folder below `previous`.
fn entered_child(previous: &str, current: &str) -> bool {
    const SEPARATORS: [char; 2] = ['/', '\\'];
    let parent = previous.trim_end_matches(SEPARATORS);
    if parent.is_empty() {
        return false;
    }
    current
        .strip_prefix(parent)
        .and_then(|rest| rest.strip_prefix(SEPARATORS))
        .is_some_and(|name| !name.is_empty() && !name.contains(SEPARATORS))
}

#[cfg(test)]
#[path = "tests/breadcrumb_glide.rs"]
mod tests;
