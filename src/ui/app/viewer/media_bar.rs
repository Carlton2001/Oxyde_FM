//! Player bar: play/pause, elapsed time, a seekable progress track and the
//! duration. Seeking follows the pointer while the left button is held.
use super::{render::icon_button, Viewer};
use crate::ui::{i18n::Message, theme::ThemeContext};
use gpui::{prelude::*, *};

pub(super) fn clock(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

impl Viewer {
    fn seek_at(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let track = self.seek_track;
        if track.size.width > px(0.0) {
            let fraction = (position.x - track.origin.x) / track.size.width;
            self.seek_media(fraction, cx);
        }
    }

    pub(super) fn render_media_bar(&self, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let (position, duration) = self
            .media
            .as_ref()
            .map(|media| (media.control.position_ms(), media.control.duration_ms()))
            .unwrap_or_default();
        let fraction = if duration > 0 { (position as f32 / duration as f32).clamp(0.0, 1.0) } else { 0.0 };
        let paused = self.media_paused();
        let handle = cx.entity();
        let track = div()
            .id("viewer-seek")
            .flex_1()
            .min_w_0()
            .h(rems(1.25))
            .flex()
            .items_center()
            .child(
                div()
                    .w_full()
                    .h(rems(0.25))
                    .rounded_full()
                    .bg(theme.scrollbar_track)
                    .child(div().h_full().w(relative(fraction)).rounded_full().bg(theme.accent)),
            )
            .child({
                let measure = handle.clone();
                canvas(|bounds, _, _| bounds, move |_, bounds, window, cx| {
                    measure.update(cx, |this, _| this.seek_track = bounds);
                    let moving = measure.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase != DispatchPhase::Capture || !moving.read(cx).seeking {
                            return;
                        }
                        moving.update(cx, |this, cx| {
                            if event.dragging() {
                                this.seek_at(event.position, cx);
                            } else {
                                this.seeking = false;
                            }
                        });
                    });
                    let release = measure.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            release.update(cx, |this, _| this.seeking = false);
                        }
                    });
                })
                .absolute()
                .size_full()
            })
            .relative()
            .when(duration > 0, |track| {
                track.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.seeking = true;
                        this.seek_at(event.position, cx);
                    }),
                )
            });
        div()
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .gap(rems(0.5))
            .px(rems(0.5))
            .py(rems(0.25))
            .bg(theme.surface)
            .border_t(rems(0.0625))
            .border_color(theme.border)
            .text_color(theme.text_muted)
            .child(
                icon_button(
                    "viewer-play",
                    if paused { "icons/play.svg" } else { "icons/pause.svg" },
                    if paused { Message::ViewerPlay } else { Message::ViewerPause },
                    true,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_media(cx))),
            )
            .child(div().flex_none().child(clock(position)))
            .child(track)
            .child(div().flex_none().child(if duration > 0 { clock(duration) } else { "--:--".into() }))
    }
}

#[cfg(test)]
mod tests {
    use super::clock;

    #[test]
    fn clock_shows_hours_only_when_needed() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(61_999), "1:01");
        assert_eq!(clock(3_725_000), "1:02:05");
    }
}
