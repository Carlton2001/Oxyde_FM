use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

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
    body: AnyElement,
    actions: Vec<AnyElement>,
    width: AbsoluteLength,
    body_gap: AbsoluteLength,
    on_backdrop_dismiss: Option<Box<dyn Fn(&mut Window, &mut App)>>,
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
            body: body.into_any_element(),
            actions: Vec::new(),
            width: rems(27.5).into(),
            body_gap: rems(0.75).into(),
            on_backdrop_dismiss: None,
        }
    }

    pub fn width(mut self, width: impl Into<AbsoluteLength>) -> Self {
        self.width = width.into();
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
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.modal_backdrop)
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
                    .w(self.width)
                    .max_w(window.viewport_size().width - rem_pixels(rems(2.0)))
                    .max_h(window.viewport_size().height - rem_pixels(rems(2.0)))
                    .flex()
                    .flex_col()
                    .p(rems(1.25))
                    .rounded_lg()
                    .bg(theme.surface)
                    .text_color(theme.text)
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(rems(0.5))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(rems(1.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.title),
                            ),
                    )
                    .child(
                        div()
                            .id("modal-body")
                            .min_h_0()
                            .overflow_y_scroll()
                            .mt(self.body_gap)
                            .child(self.body),
                    )
                    .when(!self.actions.is_empty(), |panel| {
                        panel.child(
                            div()
                                .flex_none()
                                .mt(rems(1.25))
                                .flex()
                                .justify_end()
                                .gap(rems(0.5))
                                .children(self.actions),
                        )
                    }),
            )
    }
}
