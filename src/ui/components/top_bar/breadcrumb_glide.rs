//! Glide the breadcrumb toward its end when entering a subfolder; other jumps stay instant.
use gpui::*;
use std::{cell::Cell, time::Instant};

const DURATION: f32 = 0.12;

type Glide = Cell<Option<(Instant, f32)>>;

pub(super) fn start(glide: &Glide, previous: &str, current: &str, scroll: &ScrollHandle, cx: &App) {
    let entered = !cx.reduce_motion() && entered_child(previous, current);
    glide.set(entered.then(|| (Instant::now(), scroll.offset().x.as_f32())));
}

/// Moves the scroll offset one frame forward; returns whether a glide is running.
pub(super) fn advance(glide: &Glide, scroll: &ScrollHandle, window: &Window) -> bool {
    let Some((started, from)) = glide.get() else {
        return false;
    };
    let target = -scroll.max_offset().x.as_f32();
    let t = (started.elapsed().as_secs_f32() / DURATION).min(1.0);
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
