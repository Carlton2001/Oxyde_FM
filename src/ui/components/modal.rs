use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

/// Standard compact label/value row for internal popup forms.
pub fn form_row(label: SharedString, value: impl IntoElement, cx: &App) -> impl IntoElement {
    form_row_with_label_width(label, value, rems(5.5), cx)
}

/// Forms with longer labels can reserve their own label column.
pub fn form_row_with_label_width(
    label: SharedString,
    value: impl IntoElement,
    width: Rems,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(rems(0.75))
        .child(
            div()
                .w(width)
                .flex_none()
                .text_size(rems(0.8125))
                .text_color(cx.theme().text_muted)
                .child(label),
        )
        .child(div().flex_1().min_w_0().child(value))
}

/// Compact action dimensions shared by every application modal.
pub fn action_button(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .px(rems(0.75))
        .py(rems(0.375))
        .text_size(rems(0.8125))
        .rounded_md()
}
#[derive(Default)]
struct ModalPlacement {
    offset: Point<Pixels>,
    size: Size<Pixels>,
    drag: Option<(Point<Pixels>, Point<Pixels>)>,
}

fn constrain_offset(
    offset: Point<Pixels>,
    modal: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Point<Pixels> {
    let x = ((viewport.width - modal.width) / 2.0).max(px(0.0));
    let y = ((viewport.height - modal.height) / 2.0).max(px(0.0));
    point(offset.x.clamp(-x, x), offset.y.clamp(-y, y))
}
/// Shared presentation for centered dialogs. The caller owns business state,
/// action callbacks, initial focus, keyboard bindings and placement in the
/// render tree (including `deferred` and ordering relative to context menus).
///
/// The backdrop always blocks mouse input. Dismissal is opt-in and only reacts
/// to the left button. No Escape/Enter binding or focus change is implicit.
#[derive(IntoElement)]
pub struct Modal {
    id: ElementId,
    title: SharedString,
    title_icon: Option<SharedString>,
    body: AnyElement,
    actions: Vec<AnyElement>,
    width: AbsoluteLength,
    body_gap: AbsoluteLength,
    on_backdrop_dismiss: Option<Box<dyn Fn(&mut Window, &mut App)>>,
    header: Option<AnyElement>,
}

impl Modal {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        body: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            title_icon: None,
            body: body.into_any_element(),
            actions: Vec::new(),
            width: rems(27.5).into(),
            body_gap: rems(0.375).into(),
            on_backdrop_dismiss: None,
            header: None,
        }
    }

    pub fn title_icon(mut self, path: impl Into<SharedString>) -> Self {
        self.title_icon = Some(path.into());
        self
    }
    pub fn width(mut self, width: impl Into<AbsoluteLength>) -> Self {
        self.width = width.into();
        self
    }

    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }
    pub fn body_gap(mut self, gap: impl Into<AbsoluteLength>) -> Self {
        self.body_gap = gap.into();
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    pub fn on_backdrop_dismiss(
        mut self,
        callback: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_backdrop_dismiss = Some(Box::new(callback));
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let placement =
            window.use_keyed_state(self.id.clone(), cx, |_, _| ModalPlacement::default());
        let viewport = window.viewport_size();
        let offset = constrain_offset(placement.read(cx).offset, placement.read(cx).size, viewport);
        let title_drag = placement.clone();
        let measure = placement.clone();
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .on_any_mouse_down(move |event, window, cx| {
                cx.stop_propagation();
                if event.button == MouseButton::Left {
                    if let Some(dismiss) = &self.on_backdrop_dismiss {
                        dismiss(window, cx);
                    }
                }
            })
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id(self.id)
                    .relative()
                    .left(offset.x)
                    .top(offset.y)
                    .w(self.width)
                    .max_w((viewport.width - rem_pixels(rems(2.0))).max(px(0.0)))
                    .max_h((viewport.height - rem_pixels(rems(2.0))).max(px(0.0)))
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .shadow(crate::ui::style::rem_shadow(true))
                    .bg(theme.surface)
                    .text_color(theme.text)
                    .text_size(rems(0.8125))
                    .line_height(relative(1.25))
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .child(
                        canvas(
                            |bounds, _, _| bounds,
                            move |bounds, _, window, cx| {
                                measure.update(cx, |state, cx| {
                                    let border = rem_pixels(rems(0.125));
                                    state.size = bounds.size + size(border, border);
                                    let offset = constrain_offset(
                                        state.offset,
                                        state.size,
                                        window.viewport_size(),
                                    );
                                    if offset != state.offset {
                                        state.offset = offset;
                                        cx.notify();
                                    }
                                });
                                let moving = measure.clone();
                                window.on_mouse_event(
                                    move |event: &MouseMoveEvent, phase, window, cx| {
                                        if phase == DispatchPhase::Capture {
                                            moving.update(cx, |state, cx| {
                                                if let Some((start, origin)) = state.drag {
                                                    if event.dragging() {
                                                        state.offset = constrain_offset(
                                                            origin + event.position - start,
                                                            state.size,
                                                            window.viewport_size(),
                                                        );
                                                    } else {
                                                        state.drag = None;
                                                    }
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    },
                                );
                                let released = measure.clone();
                                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                                    if phase == DispatchPhase::Capture
                                        && event.button == MouseButton::Left
                                    {
                                        released.update(cx, |state, _| {
                                            state.drag = None;
                                        });
                                    }
                                });
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .id("modal-titlebar")
                            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                title_drag.update(cx, |state, _| {
                                    state.drag = Some((event.position, state.offset));
                                });
                            })
                            .flex_none()
                            .h(rems(2.75))
                            .px(rems(1.0))
                            .flex()
                            .items_center()
                            .bg(theme.header)
                            .rounded_t_lg()
                            .border_b(rems(0.0625))
                            .border_color(theme.border)
                            .child(self.header.unwrap_or_else(|| {
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .gap(rems(0.5))
                                    .when_some(self.title_icon, |header, icon| {
                                        header.child(
                                            svg()
                                                .path(icon)
                                                .size(rems(1.0))
                                                .text_color(theme.text_muted),
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(rems(0.875))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(self.title),
                                    )
                                    .into_any_element()
                            })),
                    )
                    .child(
                        div()
                            .id("modal-body")
                            .min_h_0()
                            .overflow_y_scroll()
                            .px(rems(1.0))
                            .py(rems(1.25))
                            .flex()
                            .flex_col()
                            .gap(self.body_gap)
                            .child(self.body),
                    )
                    .when(!self.actions.is_empty(), |panel| {
                        panel.child(
                            div()
                                .flex_none()
                                .px(rems(1.0))
                                .py(rems(0.625))
                                .border_t(rems(0.0625))
                                .border_color(theme.border)
                                .flex()
                                .justify_end()
                                .gap(rems(0.5))
                                .children(self.actions),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::constrain_offset;
    use gpui::{point, px, size};

    #[test]
    fn dragging_keeps_every_edge_inside_the_viewport() {
        let modal = size(px(480.0), px(360.0));
        let viewport = size(px(1000.0), px(800.0));
        assert_eq!(
            constrain_offset(point(px(900.0), px(-900.0)), modal, viewport),
            point(px(260.0), px(-220.0))
        );
        assert_eq!(
            constrain_offset(point(px(-900.0), px(900.0)), modal, viewport),
            point(px(-260.0), px(220.0))
        );
        assert_eq!(
            constrain_offset(point(px(50.0), px(60.0)), modal, viewport),
            point(px(50.0), px(60.0))
        );
    }

    #[test]
    fn resizing_or_growing_a_modal_reclamps_its_position() {
        assert_eq!(
            constrain_offset(
                point(px(200.0), px(200.0)),
                size(px(480.0), px(500.0)),
                size(px(600.0), px(600.0))
            ),
            point(px(60.0), px(50.0))
        );
        assert_eq!(
            constrain_offset(
                point(px(200.0), px(-200.0)),
                size(px(700.0), px(700.0)),
                size(px(600.0), px(600.0))
            ),
            point(px(0.0), px(0.0))
        );
    }
}
