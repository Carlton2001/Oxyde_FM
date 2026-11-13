//! A resize separator: a 1 px line, a wider hit area straddling it and an accent that
//! fades in over the line once the pointer rests on it, or at once while it is dragged.
use crate::ui::elements::motion;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const LINE: f32 = 0.0625;
/// The hit area: 3 px on each side of the line.
const HIT: f32 = 0.4375;
/// The accent: 1 px on each side of the line.
const ACCENT: f32 = 0.1875;
/// A pointer crossing the separator does not light it.
const HOVER_DELAY: Duration = Duration::from_millis(250);

type PressHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;

/// `axis` is the direction of the drag, as a split's axis: `Horizontal` draws a
/// vertical line between panes side by side.
pub(crate) fn resize_separator(id: impl Into<SharedString>, axis: Axis) -> ResizeSeparator {
    ResizeSeparator { id: id.into(), axis, overlap: false, dragging: false, on_press: None }
}

#[derive(IntoElement)]
pub(crate) struct ResizeSeparator {
    id: SharedString,
    axis: Axis,
    overlap: bool,
    dragging: bool,
    on_press: Option<PressHandler>,
}

impl ResizeSeparator {
    /// The line takes no room and covers the last pixel of the area before it.
    pub(crate) fn overlap(mut self) -> Self {
        self.overlap = true;
        self
    }

    /// Lit while the caller's drag lasts, wherever the pointer is.
    pub(crate) fn dragging(mut self, dragging: bool) -> Self {
        self.dragging = dragging;
        self
    }

    /// A left press on the hit area, propagation already stopped.
    pub(crate) fn on_press(mut self, handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ResizeSeparator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let rested = window
            .use_keyed_state(SharedString::from(format!("{}-rest", self.id)), cx, |_, _| Rc::new(Cell::new(None::<Instant>)))
            .read(cx)
            .clone();
        let view = window.current_view();
        let lit = self.dragging || rested.get().is_some_and(|since| since.elapsed() >= HOVER_DELAY);
        let vertical_line = self.axis == Axis::Horizontal;
        let (along, across) = (relative(1.0), rems(LINE));
        let line = div().relative().flex_none().bg(theme.border);
        let line = if vertical_line {
            line.w(across).h(along).when(self.overlap, |line| line.ml(rems(-LINE)))
        } else {
            line.h(across).w(along).when(self.overlap, |line| line.mt(rems(-LINE)))
        };
        let side = (HIT - LINE) / 2.0;
        let accent = div().absolute().bg(theme.accent);
        let accent = if vertical_line {
            accent.top_0().bottom_0().left(rems((HIT - ACCENT) / 2.0)).w(rems(ACCENT))
        } else {
            accent.left_0().right_0().top(rems((HIT - ACCENT) / 2.0)).h(rems(ACCENT))
        };
        let accent = accent.with_spring(
            SharedString::from(format!("{}-accent", self.id)),
            motion::presence(lit).from(0.0),
            |accent, progress| accent.opacity(progress),
        );
        let hit = div().id(self.id.clone()).absolute().occlude();
        let hit = if vertical_line {
            hit.top_0().bottom_0().left(rems(-side)).w(rems(HIT)).cursor(CursorStyle::ResizeLeftRight)
        } else {
            hit.left_0().right_0().top(rems(-side)).h(rems(HIT)).cursor(CursorStyle::ResizeUpDown)
        };
        let hit = hit
            .child(accent)
            .on_hover(move |hovered, window, cx| {
                if !*hovered {
                    rested.set(None);
                    cx.notify(view);
                    return;
                }
                let since = Instant::now();
                rested.set(Some(since));
                let rested = rested.clone();
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor().timer(HOVER_DELAY).await;
                        if rested.get() == Some(since) {
                            cx.update(|_, cx| cx.notify(view)).ok();
                        }
                    })
                    .detach();
            })
            .when_some(self.on_press, |hit, on_press| {
                hit.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    cx.stop_propagation();
                    on_press(event, window, cx);
                })
            });
        line.child(deferred(hit))
    }
}
