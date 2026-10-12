use crate::ui::workspace::TabStripState;
use gpui::*;
use std::time::{Duration, Instant};

pub(super) fn smooth_scroll(state: TabStripState, delta: Pixels, window: &mut Window, cx: &App) {
    let start = state.scroll.offset().x.as_f32();
    let target = (state.animation_target.get().unwrap_or(start) + delta.as_f32())
        .clamp(-state.scroll.max_offset().x.as_f32(), 0.0);
    let generation = state.animation_generation.get().wrapping_add(1);
    state.animation_generation.set(generation);
    state.animation_target.set(Some(target));
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let started = Instant::now();
        loop {
            if state.animation_generation.get() != generation {
                break;
            }
            let progress = (started.elapsed().as_secs_f32() / 0.18).min(1.0);
            let eased = 1.0 - (1.0 - progress).powi(3);
            let x = start + (target - start) * eased;
            if window
                .update(cx, |_, window, _| {
                    state.scroll.set_offset(point(
                        px(x).clamp(-state.scroll.max_offset().x, px(0.0)),
                        px(0.0),
                    ));
                    window.refresh();
                })
                .is_err()
            {
                break;
            }
            if progress >= 1.0 {
                state.animation_target.set(None);
                break;
            }
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
        }
    })
    .detach();
}
