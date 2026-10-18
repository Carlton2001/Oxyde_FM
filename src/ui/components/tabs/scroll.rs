use crate::ui::workspace::TabStripState;
use gpui::*;
use std::time::Instant;

const DURATION: f32 = 0.18;

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
