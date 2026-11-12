use crate::ui::scale::logical;
use crate::ui::workspace::TabStripState;
use gpui::*;
use std::time::Instant;

const DURATION: f32 = 0.18;
/// Width of the band inside each end of the strip where a drag starts scrolling it.
const DRAG_EDGE: f32 = 30.0;
/// Scroll speed per pixel of depth into the band, per second.
const DRAG_RATE: f32 = 20.0;
/// Depth beyond which the speed stops growing, reached over the arrows.
const DRAG_MAX_DEPTH: f32 = 60.0;

/// How deep `x` is in the band along the strip's left (negative) or right (positive)
/// end, zero between the bands.
fn drag_depth(x: f32, left: f32, right: f32) -> f32 {
    let edge = logical(DRAG_EDGE);
    let depth = if x < left + edge {
        x - (left + edge)
    } else if x > right - edge {
        x - (right - edge)
    } else {
        0.0
    };
    depth.clamp(-logical(DRAG_MAX_DEPTH), logical(DRAG_MAX_DEPTH))
}

/// Scrolls an overflowing strip while a dragged tab or file nears one of its ends, as
/// rectangle selection scrolls the list, to reach the tabs out of view. Call it with
/// every drag movement over the bar, and [`stop_drag_scroll`] when the drag leaves it.
pub(crate) fn drag_scroll(state: &TabStripState, x: Pixels, window: &mut Window) {
    let viewport = state.scroll.bounds();
    let depth = if state.scroll.max_offset().x > px(0.0) {
        drag_depth(x.as_f32(), viewport.left().as_f32(), viewport.right().as_f32())
    } else {
        0.0
    };
    if depth == 0.0 {
        state.drag_scroll.set(None);
        return;
    }
    let running = state.drag_scroll.get();
    state.drag_scroll.set(Some((depth, running.map_or_else(Instant::now, |(_, at)| at))));
    if running.is_none() {
        // Cancel a gliding wheel or arrow scroll: the drag takes over.
        state.animation_generation.set(state.animation_generation.get().wrapping_add(1));
        state.animation_target.set(None);
        let state = state.clone();
        window.on_next_frame(move |window, cx| drag_step(state, window, cx));
    }
}

pub(crate) fn stop_drag_scroll(state: &TabStripState) {
    state.drag_scroll.set(None);
}

/// One frame of scrolling, integrated over the elapsed time; a still pointer sends no
/// event, so each frame asks for the next until the pointer leaves the band or drops.
fn drag_step(state: TabStripState, window: &mut Window, cx: &mut App) {
    if state.drag_scroll.get().is_none() {
        return;
    }
    if !cx.has_active_drag() {
        state.drag_scroll.set(None);
        return;
    }
    // The tabs moved under a still pointer since the last frame: replay its position
    // so the targeted slot, and the room opened for it, follow the scroll.
    let position = window.mouse_position();
    let modifiers = window.modifiers();
    window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent {
        position,
        pressed_button: Some(MouseButton::Left),
        modifiers,
    }), cx);
    let Some((depth, at)) = state.drag_scroll.get() else { return };
    let now = Instant::now();
    let elapsed = now.duration_since(at).as_secs_f32().min(0.1);
    state.drag_scroll.set(Some((depth, now)));
    let offset = (state.scroll.offset().x.as_f32() - depth * DRAG_RATE * elapsed)
        .clamp(-state.scroll.max_offset().x.as_f32(), 0.0);
    state.scroll.set_offset(point(px(offset), px(0.0)));
    window.refresh();
    window.on_next_frame(move |window, cx| drag_step(state, window, cx));
}

/// Scrolls the strip by `delta`, one step per displayed frame.
pub(super) fn smooth_scroll(state: TabStripState, delta: Pixels, window: &mut Window, _cx: &App) {
    let start = state.scroll.offset().x.as_f32();
    let target = (state.animation_target.get().unwrap_or(start) + delta.as_f32())
        .clamp(-state.scroll.max_offset().x.as_f32(), 0.0);
    let generation = state.animation_generation.get().wrapping_add(1);
    state.animation_generation.set(generation);
    state.animation_target.set(Some(target));
    step(state, generation, start, target, Instant::now(), window);
}

fn step(state: TabStripState, generation: u64, start: f32, target: f32, started: Instant, window: &mut Window) {
    if state.animation_generation.get() != generation {
        return;
    }
    let progress = (started.elapsed().as_secs_f32() / DURATION).min(1.0);
    let eased = 1.0 - (1.0 - progress).powi(3);
    let x = start + (target - start) * eased;
    state.scroll.set_offset(point(
        px(x).clamp(-state.scroll.max_offset().x, px(0.0)),
        px(0.0),
    ));
    window.refresh();
    if progress >= 1.0 {
        state.animation_target.set(None);
    } else {
        window.on_next_frame(move |window, _| step(state, generation, start, target, started, window));
    }
}

#[cfg(test)]
#[path = "tests/drag_scroll.rs"]
mod tests;
